use {
    crate::{config::IndexerConfig, db::uniswap_v3 as db},
    alloy::{
        primitives::Address,
        providers::Provider,
        rpc::types::{BlockNumberOrTag, Filter, FilterSet, Log},
        sol_types::SolEvent,
    },
    anyhow::{Context, Result},
    contracts::{
        ERC20,
        IUniswapV3Factory::IUniswapV3Factory::PoolCreated,
        UniswapV3Pool::UniswapV3Pool::{Burn, Initialize, Mint, Swap},
    },
    ethrpc::AlloyProvider,
    futures::{StreamExt, TryStreamExt},
    sqlx::PgPool,
    std::collections::HashMap,
    tracing::instrument,
};

/// Cached liquidity value keyed by (pool_address, block_number).
type LiquidityCache = HashMap<(Address, u64), u128>;
/// Cached ERC-20 decimal value keyed by token address.
type DecimalsCache = HashMap<Address, u8>;

/// Data for a newly discovered pool, sourced from a `PoolCreated` factory
/// event.
pub struct NewPoolData {
    pub address: Address,
    pub token0: Address,
    pub token1: Address,
    /// Raw fee in hundredths of a basis point (e.g. 3000 = 0.3 %).
    pub fee: u32,
    pub token0_decimals: Option<u8>,
    pub token1_decimals: Option<u8>,
    pub token0_symbol: Option<String>,
    pub token1_symbol: Option<String>,
    pub created_block: u64,
}

/// Full pool state as of a given block, sourced from an `Initialize` or `Swap`
/// event (both carry the current price, liquidity, and tick).
pub struct PoolStateData {
    pub pool_address: Address,
    pub block_number: u64,
    pub sqrt_price_x96: alloy::primitives::aliases::U160,
    pub liquidity: u128,
    pub tick: i32,
}

/// A liquidity-only pool update sourced from a `Mint` or `Burn` event when no
/// `Swap` or `Initialize` has been seen for the pool in the same chunk.
pub struct LiquidityUpdateData {
    pub pool_address: Address,
    pub block_number: u64,
    pub liquidity: u128,
}

/// A signed liquidity delta for a single tick boundary, accumulated from
/// `Mint` (+amount) and `Burn` (-amount) events.
pub struct TickDeltaData {
    pub pool_address: Address,
    pub tick_idx: i32,
    /// Net signed change to `liquidity_net` at this tick.
    pub delta: i128,
}

/// All state changes extracted from a single block-range chunk of logs,
/// ready to be written to the database in one transaction.
struct ChunkChanges {
    new_pools: Vec<NewPoolData>,
    /// Full state updates (from `Initialize` / `Swap`).
    pool_states: Vec<PoolStateData>,
    /// Liquidity-only updates (from `Mint`/`Burn` with no `Swap` in this
    /// chunk).
    liquidity_updates: Vec<LiquidityUpdateData>,
    /// Accumulated tick deltas.
    tick_deltas: Vec<TickDeltaData>,
}

/// Indexes Uniswap V3 events for a single factory contract, persisting pool
/// state and tick liquidity to the database.
pub struct UniswapV3Indexer {
    provider: AlloyProvider,
    db: PgPool,
    chain_id: u64,
    factory: Address,
    chunk_size: u64,
    finality_tag: BlockNumberOrTag,
    fetch_concurrency: usize,
    prefetch_concurrency: usize,
}

impl UniswapV3Indexer {
    pub fn new(provider: AlloyProvider, db: PgPool, config: &IndexerConfig) -> Self {
        Self {
            provider,
            db,
            chain_id: config.chain_id,
            factory: config.factory_address,
            chunk_size: config.chunk_size,
            finality_tag: if config.use_latest {
                BlockNumberOrTag::Latest
            } else {
                BlockNumberOrTag::Finalized
            },
            fetch_concurrency: config.fetch_concurrency,
            prefetch_concurrency: config.prefetch_concurrency,
        }
    }

    pub async fn run(self, poll_interval: std::time::Duration) -> ! {
        tokio::spawn(backfill_symbols(
            self.provider.clone(),
            self.db.clone(),
            self.chain_id,
            self.prefetch_concurrency,
            poll_interval,
        ));
        loop {
            if let Err(err) = self.run_once().await {
                tracing::error!(?err, "indexer error, retrying after poll interval");
            }
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Processes all pending blocks starting from `from_block` up to the
    /// current finalized block and returns. Loops until no further blocks
    /// remain (handles new blocks finalizing during a long catch-up).
    ///
    /// If no checkpoint exists yet (fresh DB after seeding), the checkpoint is
    /// initialized to `from_block - 1` so that `run_once` starts at
    /// `from_block`. This means the caller (seeder) does not need to write the
    /// checkpoint itself — it advances naturally per-chunk as blocks are
    /// indexed.
    pub async fn catch_up(&self, from_block: u64) -> Result<()> {
        let mut tx = self.db.begin().await.context("begin checkpoint tx")?;
        db::set_checkpoint(
            &mut tx,
            self.chain_id,
            &self.factory,
            from_block.saturating_sub(1),
        )
        .await?;
        tx.commit().await.context("commit checkpoint")?;
        loop {
            let finalized = self
                .provider
                .get_block_by_number(self.finality_tag)
                .await
                .context("get finalized block")?
                .context("no finalized block")?
                .header
                .number;
            let last = db::get_checkpoint(&self.db, self.chain_id, &self.factory)
                .await?
                .unwrap_or(0);
            if last >= finalized {
                tracing::info!(block = finalized, "caught up to finalized block");
                return Ok(());
            }
            self.run_once().await?;
        }
    }

    async fn run_once(&self) -> Result<()> {
        let finalized = self
            .provider
            .get_block_by_number(self.finality_tag)
            .await
            .context("get finalized block")?
            .context("no finalized block")?
            .header
            .number;

        let last_indexed = db::get_checkpoint(&self.db, self.chain_id, &self.factory)
            .await?
            .unwrap_or(0);

        if last_indexed >= finalized {
            return Ok(());
        }

        let mut chunks = Vec::new();
        let mut start = last_indexed + 1;
        while start <= finalized {
            let end = (start + self.chunk_size - 1).min(finalized);
            chunks.push((start, end));
            start = end + 1;
        }

        // Fetch chunks' logs in parallel; commit in order.
        futures::stream::iter(chunks)
            .map(|(start, end)| async move {
                let logs = self.fetch_logs_bisecting(start, end).await?;
                Ok::<_, anyhow::Error>((start, end, logs))
            })
            .buffered(self.fetch_concurrency)
            .try_for_each(|(start, end, logs)| self.commit_chunk(start, end, logs))
            .await
    }

    /// Fetches logs for `[from, to]`, sequentially bisecting on
    /// results-overflow errors. Bisection is sequential within a chunk to
    /// avoid exponential RPC fan-out; the outer `buffered` layer provides
    /// cross-chunk concurrency.
    fn fetch_logs_bisecting(
        &self,
        from: u64,
        to: u64,
    ) -> futures::future::BoxFuture<'_, Result<Vec<Log>>> {
        Box::pin(async move {
            match self.fetch_logs(from, to).await {
                Ok(logs) => Ok(logs),
                Err(err) if is_range_too_large(&err) && to > from => {
                    let mid = (from + to) / 2;
                    tracing::debug!(from, to, mid, "range too large, bisecting");
                    let mut left = self.fetch_logs_bisecting(from, mid).await?;
                    let right = self.fetch_logs_bisecting(mid + 1, to).await?;
                    left.extend(right);
                    Ok(left)
                }
                Err(err) => Err(err),
            }
        })
    }

    #[instrument(skip(self, logs), fields(chunk_start, chunk_end))]
    async fn commit_chunk(&self, chunk_start: u64, chunk_end: u64, logs: Vec<Log>) -> Result<()> {
        // Pre-fetch all I/O (liquidity + decimals eth_calls) in parallel before
        // opening the DB transaction. Symbols are intentionally excluded — a
        // hung `symbol()` call must never block pool inserts. They're populated
        // later by the async backfill task.
        let (liq_cache, dec_cache) = tokio::join!(
            self.prefetch_liquidities(&logs),
            self.prefetch_decimals(&logs),
        );

        let changes = self.collect_changes(&logs, &liq_cache, &dec_cache);

        tracing::debug!(
            chunk_start,
            chunk_end,
            log_count = logs.len(),
            new_pools = changes.new_pools.len(),
            pool_states = changes.pool_states.len(),
            liq_updates = changes.liquidity_updates.len(),
            tick_deltas = changes.tick_deltas.len(),
            "processing chunk"
        );

        let mut tx = self.db.begin().await.context("begin transaction")?;
        db::batch_insert_pools(&mut tx, self.chain_id, &changes.new_pools).await?;
        db::batch_upsert_pool_states(&mut tx, self.chain_id, &changes.pool_states).await?;
        db::batch_update_pool_liquidity(&mut tx, self.chain_id, &changes.liquidity_updates).await?;
        db::batch_update_ticks(&mut tx, self.chain_id, &changes.tick_deltas).await?;
        db::set_checkpoint(&mut tx, self.chain_id, &self.factory, chunk_end).await?;
        tx.commit().await.context("commit transaction")?;

        Ok(())
    }

    /// Collect all state changes from a set of logs into in-memory structures.
    /// This is pure computation — all I/O was done during the prefetch phase.
    fn collect_changes(
        &self,
        logs: &[Log],
        liq_cache: &LiquidityCache,
        dec_cache: &DecimalsCache,
    ) -> ChunkChanges {
        collect_log_changes(self.factory, logs, liq_cache, dec_cache)
    }

    /// Parallel-fetch liquidity for every unique (pool, block) pair from
    /// Mint/Burn events.
    async fn prefetch_liquidities(&self, logs: &[Log]) -> LiquidityCache {
        let pairs: std::collections::HashSet<_> = logs
            .iter()
            .filter_map(|log| {
                let t = log.topic0()?;
                if *t == Mint::SIGNATURE_HASH || *t == Burn::SIGNATURE_HASH {
                    Some((log.address(), log.block_number?))
                } else {
                    None
                }
            })
            .collect();

        futures::stream::iter(pairs)
            .map(|(addr, block)| async move {
                let liq = fetch_pool_liquidity(&self.provider, addr, block).await;
                ((addr, block), liq)
            })
            .buffer_unordered(self.prefetch_concurrency)
            .filter_map(|(key, opt)| async move { opt.map(|v| (key, v)) })
            .collect()
            .await
    }

    /// Parallel-fetch ERC-20 decimals for all tokens referenced in PoolCreated
    /// events.
    async fn prefetch_decimals(&self, logs: &[Log]) -> DecimalsCache {
        futures::stream::iter(pool_created_token_addresses(self.factory, logs))
            .map(|token| async move {
                let dec = fetch_decimals(&self.provider, token).await;
                (token, dec)
            })
            .buffer_unordered(self.prefetch_concurrency)
            .filter_map(|(token, opt)| async move { opt.map(|d| (token, d)) })
            .collect()
            .await
    }

    async fn fetch_logs(&self, from: u64, to: u64) -> Result<Vec<Log>> {
        let topics = FilterSet::from_iter([
            PoolCreated::SIGNATURE_HASH,
            Initialize::SIGNATURE_HASH,
            Mint::SIGNATURE_HASH,
            Burn::SIGNATURE_HASH,
            Swap::SIGNATURE_HASH,
        ]);
        let filter = Filter::new()
            .from_block(from)
            .to_block(to)
            .event_signature(topics);

        self.provider
            .get_logs(&filter)
            .await
            .with_context(|| format!("get_logs({from}..={to})"))
    }
}

/// Sign-extends a 24-bit signed integer (alloy I24) to i32.
fn signed24_to_i32(v: alloy::primitives::aliases::I24) -> i32 {
    let raw = v.into_raw().as_limbs()[0] as u32;
    (raw << 8).cast_signed() >> 8
}

async fn fetch_pool_liquidity(provider: &AlloyProvider, pool: Address, block: u64) -> Option<u128> {
    contracts::UniswapV3Pool::Instance::new(pool, provider.clone())
        .liquidity()
        .block(block.into())
        .call()
        .await
        .ok()
}

async fn fetch_decimals(provider: &AlloyProvider, token: Address) -> Option<u8> {
    ERC20::Instance::new(token, provider.clone())
        .decimals()
        .call()
        .await
        .ok()
}

/// Periodically fills in missing `token{0,1}_symbol` values on
/// `uniswap_v3_pools`. Runs forever, sleeping `poll_interval` between passes so
/// newly-indexed pools get their symbols backfilled.
///
/// Tokens whose `symbol()` call fails (revert, decode error, empty result) are
/// persisted as the empty string so subsequent passes skip them — otherwise we
/// would hammer known-broken tokens on every tick. A process restart re-probes
/// them once (cheap, and useful if the earlier failure was transient).
async fn backfill_symbols(
    provider: AlloyProvider,
    db: sqlx::PgPool,
    chain_id: u64,
    prefetch_concurrency: usize,
    poll_interval: std::time::Duration,
) -> ! {
    loop {
        if let Err(err) = run_backfill_pass(&provider, &db, chain_id, prefetch_concurrency).await {
            tracing::warn!(?err, "token symbol backfill pass failed");
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn run_backfill_pass(
    provider: &AlloyProvider,
    db: &sqlx::PgPool,
    chain_id: u64,
    prefetch_concurrency: usize,
) -> Result<()> {
    let tokens = db::get_tokens_missing_symbols(db, chain_id)
        .await
        .context("get_tokens_missing_symbols")?;
    if tokens.is_empty() {
        return Ok(());
    }
    let total = tokens.len();
    tracing::info!(total, "backfilling token symbols");

    let mut updated = 0usize;
    let mut processed = 0usize;

    for chunk in tokens.chunks(500) {
        let symbols: Vec<(Address, String)> = futures::stream::iter(chunk.iter().copied())
            .map(|token| async move {
                // `None` → "" sentinel: marks the token as "tried and failed" so
                // the next backfill pass's `IS NULL` filter skips it.
                let sym = fetch_symbol(provider, token).await.unwrap_or_default();
                (token, sym)
            })
            .buffer_unordered(prefetch_concurrency)
            .collect()
            .await;

        for (token, symbol) in &symbols {
            match db::set_token_symbol(db, chain_id, token, symbol).await {
                Ok(()) => updated += 1,
                Err(err) => tracing::warn!(%token, ?err, "failed to backfill symbol"),
            }
        }

        processed += chunk.len();
        tracing::info!(processed, total, updated, "token symbol backfill progress");
    }

    tracing::info!(updated, total, "token symbol backfill pass complete");
    Ok(())
}

async fn fetch_symbol(provider: &AlloyProvider, token: Address) -> Option<String> {
    let sym = ERC20::Instance::new(token, provider.clone())
        .symbol()
        .call()
        .await
        .ok()?;
    // Strip null bytes — some tokens embed \x00 in their symbol which Postgres
    // rejects.
    let cleaned = sym.replace('\x00', "");
    (!cleaned.is_empty()).then_some(cleaned)
}

/// Returns true when the RPC rejects a request because the result set would
/// exceed its limit. Checks the full error chain because anyhow context wraps
/// the inner RPC error.
fn is_range_too_large(err: &anyhow::Error) -> bool {
    err.chain().any(|e| {
        let msg = e.to_string().to_lowercase();
        msg.contains("max results")
            || msg.contains("result limit")
            || msg.contains("too many results")
    })
}

/// Collects the unique set of token addresses from all `PoolCreated` events
/// emitted by `factory` in `logs`.
fn pool_created_token_addresses(
    factory: Address,
    logs: &[Log],
) -> std::collections::HashSet<Address> {
    logs.iter()
        .filter_map(|log| {
            let t = log.topic0()?;
            if *t != PoolCreated::SIGNATURE_HASH || log.address() != factory {
                return None;
            }
            let decoded = PoolCreated::decode_log(&log.inner).ok()?;
            Some([decoded.data.token0, decoded.data.token1])
        })
        .flatten()
        .collect()
}

/// Accumulates per-event-type state changes while iterating over a chunk's
/// logs.
#[derive(Default)]
struct LogAccumulator {
    new_pools: HashMap<Address, NewPoolData>,
    /// Latest full state per pool, established by `Initialize` or `Swap`.
    full_states: HashMap<Address, PoolStateData>,
    /// Liquidity-only update per pool, used when no full state exists yet in
    /// the chunk (i.e. neither `Initialize` nor `Swap` has been seen).
    liq_only: HashMap<Address, (u64, u128)>,
    /// Accumulated signed tick-liquidity deltas, keyed by `(pool, tick_idx)`.
    tick_deltas: HashMap<(Address, i32), i128>,
}

impl LogAccumulator {
    /// Records a newly discovered pool, filling decimals from the prefetch
    /// cache. Symbols are left `None` here and populated later by the
    /// background backfill task.
    fn handle_pool_created(&mut self, log: &Log, dec_cache: &DecimalsCache) {
        let Ok(decoded) = PoolCreated::decode_log(&log.inner) else {
            return;
        };
        let e = &decoded.data;
        let pool: Address = e.pool;
        let token0: Address = e.token0;
        let token1: Address = e.token1;
        let created_block = log.block_number.unwrap_or(0);
        tracing::debug!(%pool, %token0, %token1, fee = e.fee.to::<u32>(), "discovered pool");
        self.new_pools.insert(
            pool,
            NewPoolData {
                address: pool,
                token0,
                token1,
                fee: e.fee.to::<u32>(),
                token0_decimals: dec_cache.get(&token0).copied(),
                token1_decimals: dec_cache.get(&token1).copied(),
                token0_symbol: None,
                token1_symbol: None,
                created_block,
            },
        );
    }

    /// Records the initial price and tick from an `Initialize` event.
    /// Preserves any liquidity already seen for this pool earlier in the chunk.
    fn handle_initialize(&mut self, log: &Log) {
        let Ok(decoded) = Initialize::decode_log(&log.inner) else {
            return;
        };
        let e = &decoded.data;
        let pool = log.address();
        let block = log.block_number.unwrap_or(0);
        let liquidity = self
            .full_states
            .get(&pool)
            .map(|s| s.liquidity)
            .unwrap_or(0);
        self.full_states.insert(
            pool,
            PoolStateData {
                pool_address: pool,
                block_number: block,
                sqrt_price_x96: e.sqrtPriceX96,
                liquidity,
                tick: signed24_to_i32(e.tick),
            },
        );
        self.liq_only.remove(&pool);
    }

    /// Records a full pool-state update (price, liquidity, tick) from a `Swap`.
    fn handle_swap(&mut self, log: &Log) {
        let Ok(decoded) = Swap::decode_log(&log.inner) else {
            return;
        };
        let e = &decoded.data;
        let pool = log.address();
        let block = log.block_number.unwrap_or(0);
        self.full_states.insert(
            pool,
            PoolStateData {
                pool_address: pool,
                block_number: block,
                sqrt_price_x96: e.sqrtPriceX96,
                liquidity: e.liquidity,
                tick: signed24_to_i32(e.tick),
            },
        );
        self.liq_only.remove(&pool);
    }

    /// Applies positive tick-liquidity deltas from a `Mint` and refreshes
    /// pool liquidity from the prefetch cache.
    fn handle_mint(&mut self, log: &Log, liq_cache: &LiquidityCache) {
        let Ok(decoded) = Mint::decode_log(&log.inner) else {
            return;
        };
        let e = &decoded.data;
        let pool = log.address();
        let block = log.block_number.unwrap_or(0);
        let amount = e.amount.cast_signed();
        *self
            .tick_deltas
            .entry((pool, signed24_to_i32(e.tickLower)))
            .or_default() += amount;
        *self
            .tick_deltas
            .entry((pool, signed24_to_i32(e.tickUpper)))
            .or_default() -= amount;
        self.update_liquidity_from_cache(pool, block, liq_cache);
    }

    /// Applies negative tick-liquidity deltas from a `Burn` and refreshes
    /// pool liquidity from the prefetch cache.
    fn handle_burn(&mut self, log: &Log, liq_cache: &LiquidityCache) {
        let Ok(decoded) = Burn::decode_log(&log.inner) else {
            return;
        };
        let e = &decoded.data;
        let pool = log.address();
        let block = log.block_number.unwrap_or(0);
        let amount = e.amount.cast_signed();
        *self
            .tick_deltas
            .entry((pool, signed24_to_i32(e.tickLower)))
            .or_default() -= amount;
        *self
            .tick_deltas
            .entry((pool, signed24_to_i32(e.tickUpper)))
            .or_default() += amount;
        self.update_liquidity_from_cache(pool, block, liq_cache);
    }

    /// Refreshes the stored liquidity for `pool` at `block` using the
    /// prefetch cache. Updates the existing full state in-place if one exists,
    /// otherwise stores a liquidity-only record.
    fn update_liquidity_from_cache(
        &mut self,
        pool: Address,
        block: u64,
        liq_cache: &LiquidityCache,
    ) {
        if let Some(&liq) = liq_cache.get(&(pool, block)) {
            if let Some(state) = self.full_states.get_mut(&pool) {
                state.liquidity = liq;
                state.block_number = block;
            } else {
                self.liq_only.insert(pool, (block, liq));
            }
        }
    }

    fn into_chunk_changes(self) -> ChunkChanges {
        ChunkChanges {
            new_pools: self.new_pools.into_values().collect(),
            pool_states: self.full_states.into_values().collect(),
            liquidity_updates: self
                .liq_only
                .into_iter()
                .map(|(pool, (block, liq))| LiquidityUpdateData {
                    pool_address: pool,
                    block_number: block,
                    liquidity: liq,
                })
                .collect(),
            tick_deltas: self
                .tick_deltas
                .into_iter()
                .filter(|(_, d)| *d != 0)
                .map(|((pool, tick), delta)| TickDeltaData {
                    pool_address: pool,
                    tick_idx: tick,
                    delta,
                })
                .collect(),
        }
    }
}

fn collect_log_changes(
    factory: Address,
    logs: &[Log],
    liq_cache: &LiquidityCache,
    dec_cache: &DecimalsCache,
) -> ChunkChanges {
    let mut acc = LogAccumulator::default();
    for log in logs {
        let Some(t) = log.topic0() else { continue };
        match *t {
            t if t == PoolCreated::SIGNATURE_HASH && log.address() == factory => {
                acc.handle_pool_created(log, dec_cache);
            }
            t if t == Initialize::SIGNATURE_HASH => acc.handle_initialize(log),
            t if t == Swap::SIGNATURE_HASH => acc.handle_swap(log),
            t if t == Mint::SIGNATURE_HASH => acc.handle_mint(log, liq_cache),
            t if t == Burn::SIGNATURE_HASH => acc.handle_burn(log, liq_cache),
            _ => {}
        }
    }
    acc.into_chunk_changes()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{
            primitives::{
                I256,
                aliases::{I24, U24, U160},
            },
            sol_types::SolEvent,
        },
        contracts::{
            IUniswapV3Factory::IUniswapV3Factory::PoolCreated,
            UniswapV3Pool::UniswapV3Pool::{Burn, Initialize, Mint, Swap},
        },
    };

    const FACTORY: Address = Address::repeat_byte(0xFA);
    const POOL: Address = Address::repeat_byte(0x01);
    const TOKEN0: Address = Address::repeat_byte(0x02);
    const TOKEN1: Address = Address::repeat_byte(0x03);
    // sqrt(1) * 2^96 — a valid initialised price
    const SQRT_PRICE_1: u128 = 79_228_162_514_264_337_593_543_950_336;

    fn t(n: i32) -> I24 {
        I24::try_from(n).unwrap()
    }

    fn make_log(address: Address, block: u64, event: impl SolEvent) -> Log {
        Log {
            inner: alloy_primitives::Log {
                address,
                data: event.encode_log_data(),
            },
            block_number: Some(block),
            block_hash: None,
            block_timestamp: None,
            transaction_hash: None,
            transaction_index: None,
            log_index: None,
            removed: false,
        }
    }

    #[test]
    fn empty_logs_produce_empty_changes() {
        let c = collect_log_changes(FACTORY, &[], &Default::default(), &Default::default());
        assert!(c.new_pools.is_empty());
        assert!(c.pool_states.is_empty());
        assert!(c.liquidity_updates.is_empty());
        assert!(c.tick_deltas.is_empty());
    }

    #[test]
    fn pool_created_from_factory_inserted() {
        let event = PoolCreated {
            token0: TOKEN0,
            token1: TOKEN1,
            fee: U24::from(500u32),
            tickSpacing: t(10),
            pool: POOL,
        };
        let log = make_log(FACTORY, 100, event);
        let c = collect_log_changes(FACTORY, &[log], &Default::default(), &Default::default());
        assert_eq!(c.new_pools.len(), 1);
        assert_eq!(c.new_pools[0].address, POOL);
        assert_eq!(c.new_pools[0].fee, 500);
    }

    #[test]
    fn pool_created_wrong_factory_ignored() {
        let event = PoolCreated {
            token0: TOKEN0,
            token1: TOKEN1,
            fee: U24::from(500u32),
            tickSpacing: t(10),
            pool: POOL,
        };
        let log = make_log(Address::repeat_byte(0xBB), 100, event);
        let c = collect_log_changes(FACTORY, &[log], &Default::default(), &Default::default());
        assert!(c.new_pools.is_empty());
    }

    #[test]
    fn initialize_creates_full_state_with_zero_liquidity() {
        let event = Initialize {
            sqrtPriceX96: U160::from(SQRT_PRICE_1),
            tick: t(0),
        };
        let log = make_log(POOL, 100, event);
        let c = collect_log_changes(FACTORY, &[log], &Default::default(), &Default::default());
        assert_eq!(c.pool_states.len(), 1);
        assert_eq!(c.pool_states[0].pool_address, POOL);
        assert_eq!(c.pool_states[0].block_number, 100);
        assert_eq!(c.pool_states[0].tick, 0);
        assert_eq!(c.pool_states[0].liquidity, 0);
    }

    #[test]
    fn swap_creates_full_state() {
        let event = Swap {
            sender: Address::ZERO,
            recipient: Address::ZERO,
            amount0: I256::ZERO,
            amount1: I256::ZERO,
            sqrtPriceX96: U160::from(SQRT_PRICE_1),
            liquidity: 500_000u128,
            tick: t(42),
        };
        let log = make_log(POOL, 200, event);
        let c = collect_log_changes(FACTORY, &[log], &Default::default(), &Default::default());
        assert_eq!(c.pool_states.len(), 1);
        assert_eq!(c.pool_states[0].tick, 42);
        assert_eq!(c.pool_states[0].liquidity, 500_000);
        assert_eq!(c.pool_states[0].block_number, 200);
    }

    #[test]
    fn mint_produces_correct_tick_deltas_and_liq_only() {
        let amount = 1_000_000u128;
        let event = Mint {
            sender: Address::ZERO,
            owner: Address::ZERO,
            tickLower: t(-100),
            tickUpper: t(100),
            amount,
            amount0: alloy::primitives::U256::ZERO,
            amount1: alloy::primitives::U256::ZERO,
        };
        let liq_cache: LiquidityCache = HashMap::from([((POOL, 100u64), amount)]);
        let log = make_log(POOL, 100, event);
        let c = collect_log_changes(FACTORY, &[log], &liq_cache, &Default::default());

        assert_eq!(c.tick_deltas.len(), 2);
        let lower = c.tick_deltas.iter().find(|d| d.tick_idx == -100).unwrap();
        let upper = c.tick_deltas.iter().find(|d| d.tick_idx == 100).unwrap();
        assert_eq!(lower.delta, amount.cast_signed());
        assert_eq!(upper.delta, -amount.cast_signed());

        // No prior full state → goes into liq_only
        assert_eq!(c.liquidity_updates.len(), 1);
        assert_eq!(c.liquidity_updates[0].liquidity, amount);
        assert!(c.pool_states.is_empty());
    }

    #[test]
    fn mint_after_swap_updates_full_state_liquidity() {
        let swap_liq = 500_000u128;
        let after_mint_liq = 600_000u128;

        let swap = Swap {
            sender: Address::ZERO,
            recipient: Address::ZERO,
            amount0: I256::ZERO,
            amount1: I256::ZERO,
            sqrtPriceX96: U160::from(SQRT_PRICE_1),
            liquidity: swap_liq,
            tick: t(0),
        };
        let mint = Mint {
            sender: Address::ZERO,
            owner: Address::ZERO,
            tickLower: t(-100),
            tickUpper: t(100),
            amount: 100_000u128,
            amount0: alloy::primitives::U256::ZERO,
            amount1: alloy::primitives::U256::ZERO,
        };
        let liq_cache: LiquidityCache = HashMap::from([((POOL, 201u64), after_mint_liq)]);
        let logs = vec![make_log(POOL, 200, swap), make_log(POOL, 201, mint)];
        let c = collect_log_changes(FACTORY, &logs, &liq_cache, &Default::default());

        assert_eq!(c.pool_states.len(), 1);
        // Swap established full_state; Mint updated its liquidity from the cache.
        assert_eq!(c.pool_states[0].liquidity, after_mint_liq);
        assert_eq!(c.pool_states[0].block_number, 201);
        assert!(c.liquidity_updates.is_empty());
    }

    #[test]
    fn burn_zeroes_tick_filtered_out() {
        let amount = 1_000_000u128;
        let mint = Mint {
            sender: Address::ZERO,
            owner: Address::ZERO,
            tickLower: t(-100),
            tickUpper: t(100),
            amount,
            amount0: alloy::primitives::U256::ZERO,
            amount1: alloy::primitives::U256::ZERO,
        };
        let burn = Burn {
            owner: Address::ZERO,
            tickLower: t(-100),
            tickUpper: t(100),
            amount,
            amount0: alloy::primitives::U256::ZERO,
            amount1: alloy::primitives::U256::ZERO,
        };
        let logs = vec![make_log(POOL, 100, mint), make_log(POOL, 101, burn)];
        let c = collect_log_changes(FACTORY, &logs, &Default::default(), &Default::default());
        assert!(c.tick_deltas.is_empty(), "zero-net ticks must be pruned");
    }

    #[test]
    fn partial_burn_leaves_nonzero_delta() {
        let mint_amount = 1_000_000u128;
        let burn_amount = 400_000u128;
        let mint = Mint {
            sender: Address::ZERO,
            owner: Address::ZERO,
            tickLower: t(-100),
            tickUpper: t(100),
            amount: mint_amount,
            amount0: alloy::primitives::U256::ZERO,
            amount1: alloy::primitives::U256::ZERO,
        };
        let burn = Burn {
            owner: Address::ZERO,
            tickLower: t(-100),
            tickUpper: t(100),
            amount: burn_amount,
            amount0: alloy::primitives::U256::ZERO,
            amount1: alloy::primitives::U256::ZERO,
        };
        let logs = vec![make_log(POOL, 100, mint), make_log(POOL, 101, burn)];
        let c = collect_log_changes(FACTORY, &logs, &Default::default(), &Default::default());

        let expected = (mint_amount - burn_amount).cast_signed();
        let lower = c.tick_deltas.iter().find(|d| d.tick_idx == -100).unwrap();
        let upper = c.tick_deltas.iter().find(|d| d.tick_idx == 100).unwrap();
        assert_eq!(lower.delta, expected);
        assert_eq!(upper.delta, -expected);
    }

    #[test]
    fn pool_created_and_initialize_same_chunk() {
        let created = PoolCreated {
            token0: TOKEN0,
            token1: TOKEN1,
            fee: U24::from(3000u32),
            tickSpacing: t(60),
            pool: POOL,
        };
        let init = Initialize {
            sqrtPriceX96: U160::from(SQRT_PRICE_1),
            tick: t(0),
        };
        let logs = vec![make_log(FACTORY, 100, created), make_log(POOL, 100, init)];
        let c = collect_log_changes(FACTORY, &logs, &Default::default(), &Default::default());
        assert_eq!(c.new_pools.len(), 1);
        assert_eq!(c.pool_states.len(), 1);
        assert_eq!(c.pool_states[0].pool_address, POOL);
    }
}
