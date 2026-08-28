use {
    crate::{
        config::{BalancerV2Config, FactoryConfig, NetworkName},
        db::{balancer_v2 as db, get_checkpoint, set_checkpoint},
    },
    alloy_primitives::{Address, B256, U256},
    alloy_provider::Provider,
    alloy_rpc_types_eth::{BlockNumberOrTag, Log},
    alloy_sol_types::SolEvent,
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    contracts::{
        BalancerV2BasePool,
        BalancerV2BasePoolFactory::BalancerV2BasePoolFactory::PoolCreated,
        BalancerV2Vault,
        BalancerV2WeightedPool,
    },
    ethrpc::AlloyProvider,
    futures::{StreamExt, TryStreamExt},
    number::conversions::ufixed18_to_big_decimal,
    sqlx::PgPool,
    std::collections::HashMap,
    tracing::instrument,
};

const BACKFILL_BATCH_SIZE: usize = 500;

/// Balancer V2 pool type, implied by the factory group a pool was discovered
/// under. Stored as [`Self::as_str`] and served verbatim by the API; the
/// weighted V0/V3-plus distinction is recovered from the factory address, so
/// it isn't a separate type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PoolType {
    Weighted,
    Stable,
    ComposableStable,
    LiquidityBootstrapping,
}

impl PoolType {
    /// String stored in `balancer_v2_pools.pool_type`; matches the schema's
    /// `CHECK` constraint.
    pub fn as_str(self) -> &'static str {
        match self {
            PoolType::Weighted => "Weighted",
            PoolType::Stable => "Stable",
            PoolType::ComposableStable => "ComposableStable",
            PoolType::LiquidityBootstrapping => "LiquidityBootstrapping",
        }
    }

    /// Whether pools of this type expose `getNormalizedWeights`. Only weighted
    /// pools carry static normalized weights; stable/composable-stable/LBP
    /// weights are absent or computed on-chain, so they aren't fetched here.
    pub fn has_weights(self) -> bool {
        matches!(self, PoolType::Weighted)
    }
}

/// Flattens a [`BalancerV2Config`] into `(pool_type, factory)` pairs across all
/// factory groups. The pool type is implied by the group; both weighted groups
/// map to [`PoolType::Weighted`].
pub fn configured_factories(config: &BalancerV2Config) -> Vec<(PoolType, FactoryConfig)> {
    let groups: [(PoolType, &[FactoryConfig]); 5] = [
        (PoolType::Weighted, &config.weighted),
        (PoolType::Weighted, &config.weighted_v3plus),
        (PoolType::Stable, &config.stable),
        (
            PoolType::LiquidityBootstrapping,
            &config.liquidity_bootstrapping,
        ),
        (PoolType::ComposableStable, &config.composable_stable),
    ];
    groups
        .into_iter()
        .flat_map(|(pool_type, factories)| factories.iter().map(move |f| (pool_type, *f)))
        .collect()
}

/// Config for one Balancer V2 factory discovery loop.
pub struct IndexerConfig {
    pub network: NetworkName,
    pub vault: Address,
    pub factory: Address,
    pub pool_type: PoolType,
    pub deploy_block: u64,
    pub chunk_size: u64,
    pub use_latest: bool,
    pub fetch_concurrency: usize,
    pub enrich_concurrency: usize,
}

/// A token within a discovered pool, in `Vault.getPoolTokens` order.
pub struct NewPoolToken {
    pub position: usize,
    pub address: Address,
    pub decimals: Option<u8>,
    /// Normalized weight as a Bfp (1e18) fraction; `Some` only for weighted
    /// pools.
    pub weight: Option<BigDecimal>,
}

/// A pool discovered from a factory `PoolCreated` event, enriched with on-chain
/// metadata.
pub struct NewBalancerPool {
    pub pool_id: B256,
    pub address: Address,
    pub pool_type: PoolType,
    pub created_block: u64,
    pub tokens: Vec<NewPoolToken>,
}

#[derive(Clone, Copy, Debug)]
struct ChunkRange {
    start: u64,
    end: u64,
}

/// Discovers Balancer V2 pools created by one factory and persists their
/// static metadata. Dynamic state (balances, amp, swap fee, ...) is fetched
/// on-chain by the driver, not here.
pub struct BalancerV2Indexer {
    provider: AlloyProvider,
    db: PgPool,
    network: NetworkName,
    vault: Address,
    factory: Address,
    factory_label: String,
    pool_type: PoolType,
    deploy_block: u64,
    chunk_size: u64,
    finality_tag: BlockNumberOrTag,
    fetch_concurrency: usize,
    enrich_concurrency: usize,
}

impl BalancerV2Indexer {
    pub fn new(provider: AlloyProvider, db: PgPool, config: IndexerConfig) -> Self {
        Self {
            provider,
            db,
            network: config.network,
            vault: config.vault,
            factory: config.factory,
            factory_label: format!("{:#x}", config.factory),
            pool_type: config.pool_type,
            deploy_block: config.deploy_block,
            chunk_size: config.chunk_size,
            finality_tag: if config.use_latest {
                BlockNumberOrTag::Latest
            } else {
                BlockNumberOrTag::Finalized
            },
            fetch_concurrency: config.fetch_concurrency,
            enrich_concurrency: config.enrich_concurrency,
        }
    }

    /// Per-factory live-discovery loop.
    pub async fn run(self, poll_interval: std::time::Duration) -> ! {
        let mut interval = tokio::time::interval(poll_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            if let Err(err) = self.run_once().await {
                crate::metrics::Metrics::get()
                    .indexer_errors
                    .with_label_values(&[self.network.as_str(), self.factory_label.as_str()])
                    .inc();
                tracing::error!(?err, factory = %self.factory, "balancer indexer error, retrying");
            }
        }
    }

    /// Scans `PoolCreated` from the factory's deploy block to the finalized
    /// head, then returns. Idempotent: skips if a checkpoint already exists, so
    /// re-running on a seeded DB is a fast no-op.
    pub async fn bootstrap(&self) -> Result<()> {
        if get_checkpoint(&self.db, &self.factory).await?.is_some() {
            tracing::info!(factory = %self.factory, "existing checkpoint, skipping bootstrap");
            return Ok(());
        }
        // `run_once` resumes at `checkpoint + 1`, so start one before the
        // deploy block to scan the deploy block itself.
        let start = self.deploy_block.saturating_sub(1);
        let mut tx = self.db.begin().await.context("begin checkpoint tx")?;
        set_checkpoint(&mut tx, &self.factory, start).await?;
        tx.commit().await.context("commit checkpoint tx")?;

        loop {
            let finalized = self.finalized_block().await?;
            if self.last_indexed_block().await? >= finalized {
                tracing::info!(block = finalized, factory = %self.factory, "balancer bootstrap caught up");
                return Ok(());
            }
            self.run_once().await?;
        }
    }

    async fn run_once(&self) -> Result<()> {
        let finalized = self.finalized_block().await?;
        let last = self.last_indexed_block().await?;
        let lag = finalized.saturating_sub(last);
        crate::metrics::Metrics::get()
            .indexer_lag_blocks
            .with_label_values(&[self.network.as_str(), self.factory_label.as_str()])
            .set(i64::try_from(lag).unwrap_or(i64::MAX));
        if last >= finalized {
            return Ok(());
        }

        // Fetch chunks in parallel, commit in order.
        futures::stream::iter(self.pending_chunks(last, finalized))
            .map(|chunk| async move {
                let logs = self.fetch_pool_created(chunk.start, chunk.end).await?;
                Ok::<_, anyhow::Error>((chunk, logs))
            })
            .buffered(self.fetch_concurrency)
            .try_for_each(|(chunk, logs)| self.commit_chunk(chunk, logs, finalized))
            .await?;
        Ok(())
    }

    async fn finalized_block(&self) -> Result<u64> {
        Ok(self
            .provider
            .get_block_by_number(self.finality_tag)
            .await
            .context("get finalized block")?
            .context("no finalized block")?
            .header
            .number)
    }

    async fn last_indexed_block(&self) -> Result<u64> {
        Ok(get_checkpoint(&self.db, &self.factory).await?.unwrap_or(0))
    }

    fn pending_chunks(&self, last: u64, finalized: u64) -> Vec<ChunkRange> {
        let mut chunks = Vec::new();
        let mut next_start = last + 1;
        while next_start <= finalized {
            let end = (next_start + self.chunk_size - 1).min(finalized);
            chunks.push(ChunkRange {
                start: next_start,
                end,
            });
            next_start = end + 1;
        }
        chunks
    }

    /// `PoolCreated` is emitted by the factory only, so filter by the factory
    /// address — the query stays tiny even over large ranges.
    async fn fetch_pool_created(&self, from: u64, to: u64) -> Result<Vec<Log>> {
        super::bisecting_get_logs(
            &self.provider,
            from,
            to,
            vec![self.factory],
            vec![PoolCreated::SIGNATURE_HASH],
        )
        .await
    }

    #[instrument(skip(self, logs), fields(chunk_start = chunk.start, chunk_end = chunk.end))]
    async fn commit_chunk(&self, chunk: ChunkRange, logs: Vec<Log>, target: u64) -> Result<()> {
        let pools: Vec<NewBalancerPool> =
            futures::stream::iter(pool_addresses(self.factory, &logs))
                .map(|(pool, block)| self.enrich_pool(pool, block))
                .buffer_unordered(self.enrich_concurrency)
                .filter_map(|res| async move {
                    match res {
                        Ok(pool) => pool,
                        Err(err) => {
                            tracing::warn!(?err, "balancer pool enrichment failed; skipping");
                            None
                        }
                    }
                })
                .collect()
                .await;

        let network = self.network.as_str();
        let factory = self.factory_label.as_str();
        crate::metrics::Metrics::get()
            .events_applied
            .with_label_values(&[network, factory, "new_pool"])
            .inc_by(pools.len() as u64);

        let mut tx = self.db.begin().await.context("begin transaction")?;
        db::insert_pools(&mut tx, &self.factory, &pools).await?;
        set_checkpoint(&mut tx, &self.factory, chunk.end).await?;
        tx.commit().await.context("commit transaction")?;

        let metrics = crate::metrics::Metrics::get();
        metrics
            .indexed_block
            .with_label_values(&[network, factory])
            .set(i64::try_from(chunk.end).unwrap_or(i64::MAX));
        let lag = target.saturating_sub(chunk.end);
        metrics
            .indexer_lag_blocks
            .with_label_values(&[network, factory])
            .set(i64::try_from(lag).unwrap_or(i64::MAX));
        Ok(())
    }

    /// Enriches a freshly-discovered pool with its on-chain metadata:
    /// `getPoolId` → `Vault.getPoolTokens` → per-token `decimals` →
    /// `getNormalizedWeights` (weighted pools). Returns `None` if a required
    /// call fails (the pool is retried on the next pass — the checkpoint isn't
    /// advanced past it until it inserts).
    async fn enrich_pool(
        &self,
        pool: Address,
        created_block: u64,
    ) -> Result<Option<NewBalancerPool>> {
        let pool_id = match BalancerV2BasePool::Instance::new(pool, self.provider.clone())
            .getPoolId()
            .call()
            .await
        {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(%pool, ?err, "getPoolId failed; skipping pool");
                return Ok(None);
            }
        };

        let tokens = BalancerV2Vault::Instance::new(self.vault, self.provider.clone())
            .getPoolTokens(pool_id.0.into())
            .call()
            .await
            .context("getPoolTokens")?
            .tokens;

        let weights = if self.pool_type.has_weights() {
            match BalancerV2WeightedPool::Instance::new(pool, self.provider.clone())
                .getNormalizedWeights()
                .call()
                .await
            {
                Ok(weights) => Some(weights),
                Err(err) => {
                    tracing::warn!(%pool, ?err, "getNormalizedWeights failed; skipping pool");
                    return Ok(None);
                }
            }
        } else {
            None
        };

        let decimals: HashMap<Address, u8> = futures::stream::iter(tokens.clone())
            .map(|token| async move { (token, super::fetch_decimals(&self.provider, token).await) })
            .buffer_unordered(self.enrich_concurrency)
            .filter_map(|(token, decimals)| async move { decimals.map(|d| (token, d)) })
            .collect()
            .await;

        Ok(Some(assemble_pool(
            pool,
            pool_id,
            self.pool_type,
            created_block,
            tokens,
            &decimals,
            weights,
        )))
    }
}

/// Decodes factory `PoolCreated` events into `(pool address, created block)`.
/// Logs are already factory-filtered; the emitter check is defense-in-depth.
fn pool_addresses(factory: Address, logs: &[Log]) -> Vec<(Address, u64)> {
    logs.iter()
        .filter_map(|log| {
            let topic = log.topic0()?;
            if *topic != PoolCreated::SIGNATURE_HASH || log.address() != factory {
                return None;
            }
            let decoded = PoolCreated::decode_log(&log.inner).ok()?;
            Some((decoded.data.pool, log.block_number.unwrap_or_default()))
        })
        .collect()
}

/// Builds a pool row from its enrichment results. Tokens keep their
/// `getPoolTokens` order via `position`; `weights[i]` aligns with `tokens[i]`
/// and is converted from 1e18 fixed-point to a decimal fraction. Pure (no I/O)
/// so the mapping is unit-testable.
fn assemble_pool(
    pool: Address,
    pool_id: B256,
    pool_type: PoolType,
    created_block: u64,
    tokens: Vec<Address>,
    decimals: &HashMap<Address, u8>,
    weights: Option<Vec<U256>>,
) -> NewBalancerPool {
    let weights = weights.unwrap_or_default();
    let tokens = tokens
        .into_iter()
        .enumerate()
        .map(|(position, address)| NewPoolToken {
            position,
            address,
            decimals: decimals.get(&address).copied(),
            weight: weights.get(position).map(ufixed18_to_big_decimal),
        })
        .collect();
    NewBalancerPool {
        pool_id,
        address: pool,
        pool_type,
        created_block,
        tokens,
    }
}

/// Periodically fills `decimals` on pool tokens whose discovery-time
/// `decimals()` call failed (or was never made). `-1` is the "tried, failed"
/// sentinel so a known-broken token isn't probed every pass.
pub(crate) async fn backfill_decimals(
    provider: AlloyProvider,
    db: PgPool,
    network: NetworkName,
    concurrency: usize,
    poll_interval: std::time::Duration,
) {
    let mut interval = tokio::time::interval(poll_interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        if let Err(err) = run_decimals_backfill_pass(&provider, &db, &network, concurrency).await {
            tracing::error!(?err, "balancer token decimals backfill pass failed");
        }
    }
}

async fn run_decimals_backfill_pass(
    provider: &AlloyProvider,
    db: &PgPool,
    network: &NetworkName,
    concurrency: usize,
) -> Result<()> {
    let tokens = db::get_tokens_missing_decimals(db).await?;
    let network = network.as_str();
    crate::metrics::Metrics::get()
        .backfill_pending
        .with_label_values(&[network, "decimals"])
        .set(i64::try_from(tokens.len()).unwrap_or(-1));
    if tokens.is_empty() {
        return Ok(());
    }
    let total = tokens.len();
    tracing::info!(total, "backfilling balancer token decimals");

    let mut stream = futures::stream::iter(tokens)
        .map(|token| async move {
            // `None` → `-1` is the "tried, failed" sentinel; the next pass's
            // `IS NULL` filter skips it.
            let decimals = super::fetch_decimals(provider, token)
                .await
                .map(i16::from)
                .unwrap_or(-1);
            (token, decimals)
        })
        .buffer_unordered(concurrency)
        .ready_chunks(BACKFILL_BATCH_SIZE);

    let mut updated = 0usize;
    while let Some(batch) = stream.next().await {
        match write_decimals_batch(db, &batch).await {
            Ok(()) => {
                for (_, decimals) in &batch {
                    updated += 1;
                    let result = if *decimals < 0 { "empty" } else { "ok" };
                    crate::metrics::Metrics::get()
                        .backfilled
                        .with_label_values(&[network, "decimals", result])
                        .inc();
                }
            }
            Err(err) => tracing::warn!(?err, "failed to backfill balancer decimals batch"),
        }
    }
    tracing::info!(updated, total, "balancer token decimals backfill complete");
    Ok(())
}

async fn write_decimals_batch(db: &PgPool, entries: &[(Address, i16)]) -> Result<()> {
    let mut tx = db.begin().await.context("begin decimals batch tx")?;
    db::batch_set_token_decimals(&mut tx, entries).await?;
    tx.commit().await.context("commit decimals batch tx")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POOL: Address = Address::repeat_byte(0x11);
    const TOKEN0: Address = Address::repeat_byte(0x01);
    const TOKEN1: Address = Address::repeat_byte(0x02);
    // 0.5 in Balancer's 1e18 fixed point.
    const HALF: u64 = 500_000_000_000_000_000;

    #[test]
    fn assembles_weighted_pool_with_fractional_weights() {
        let pool_id = B256::repeat_byte(0x22);
        // TOKEN1 deliberately missing from the decimals cache.
        let decimals = HashMap::from([(TOKEN0, 18u8)]);
        let weights = Some(vec![U256::from(HALF), U256::from(HALF)]);

        let pool = assemble_pool(
            POOL,
            pool_id,
            PoolType::Weighted,
            100,
            vec![TOKEN0, TOKEN1],
            &decimals,
            weights,
        );

        assert_eq!(pool.pool_id, pool_id);
        assert_eq!(pool.address, POOL);
        assert_eq!(pool.created_block, 100);
        assert_eq!(pool.tokens.len(), 2);

        assert_eq!(pool.tokens[0].position, 0);
        assert_eq!(pool.tokens[0].address, TOKEN0);
        assert_eq!(pool.tokens[0].decimals, Some(18));
        assert_eq!(
            pool.tokens[0].weight.as_ref().unwrap().to_string(),
            "0.500000000000000000"
        );

        assert_eq!(pool.tokens[1].position, 1);
        assert_eq!(pool.tokens[1].decimals, None);
        assert!(pool.tokens[1].weight.is_some());
    }

    #[test]
    fn assembles_non_weighted_pool_without_weights() {
        let decimals = HashMap::from([(TOKEN0, 6u8)]);
        let pool = assemble_pool(
            POOL,
            B256::repeat_byte(0x22),
            PoolType::Stable,
            100,
            vec![TOKEN0],
            &decimals,
            None,
        );
        assert_eq!(pool.tokens.len(), 1);
        assert_eq!(pool.tokens[0].decimals, Some(6));
        assert!(pool.tokens[0].weight.is_none());
    }

    #[test]
    fn configured_factories_maps_groups_to_pool_types() {
        let f = |b| FactoryConfig {
            address: Address::ZERO,
            deploy_block: b,
        };
        let config = BalancerV2Config {
            vault: Address::ZERO,
            chunk_size: 100_000,
            weighted: vec![f(1)],
            weighted_v3plus: vec![f(2)],
            stable: vec![f(3)],
            liquidity_bootstrapping: vec![f(4)],
            composable_stable: vec![f(5)],
        };
        let got: Vec<_> = configured_factories(&config)
            .into_iter()
            .map(|(t, factory)| (t, factory.deploy_block))
            .collect();
        assert_eq!(
            got,
            vec![
                (PoolType::Weighted, 1),
                (PoolType::Weighted, 2),
                (PoolType::Stable, 3),
                (PoolType::LiquidityBootstrapping, 4),
                (PoolType::ComposableStable, 5),
            ]
        );
    }
}
