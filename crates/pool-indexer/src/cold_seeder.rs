//! Bootstraps the pool-indexer from on-chain data alone.
//!
//! Used when a chain has no Uniswap V3 subgraph. Three phases:
//!
//! 1. **Pool discovery** — scan `PoolCreated` events on the factory from
//!    genesis to `snapshot_block`.
//! 2. **State snapshot** — per-pool `slot0()` + `liquidity()` at
//!    `snapshot_block`, fanned out across concurrent eth_calls.
//! 3. **Tick reconstruction** — for pools with non-zero liquidity, filter
//!    `Mint`/`Burn` logs by pool address over the full history and accumulate
//!    `liquidity_net` deltas per tick.
//!
//! The live indexer takes over from `snapshot_block + 1` via `catch_up`.

use {
    crate::{
        db::uniswap_v3 as db,
        indexer::uniswap_v3::{NewPoolData, PoolStateData, TickDeltaData, bisecting_get_logs},
    },
    alloy::{primitives::Address, providers::Provider, rpc::types::Log, sol_types::SolEvent},
    anyhow::{Context, Result},
    contracts::{
        ERC20,
        IUniswapV3Factory::IUniswapV3Factory::PoolCreated,
        UniswapV3Pool::{
            self,
            UniswapV3Pool::{Burn, Mint},
        },
    },
    ethrpc::AlloyProvider,
    futures::{StreamExt, TryStreamExt},
    sqlx::PgPool,
    std::collections::HashMap,
    tracing::{info, instrument, warn},
};

/// Initial block-range size for `PoolCreated` discovery. Bisected on
/// "range too large" errors, so picking too small only costs extra
/// round-trips, never fails. 10k is Alchemy's per-call cap on chains like
/// Ink; more permissive endpoints could run faster with a larger value.
const DISCOVERY_BLOCK_CHUNK: u64 = 10_000;

/// Initial block-range size for Mint/Burn history scans in phase 3.
const HISTORY_BLOCK_CHUNK: u64 = 10_000;

/// Number of pools per `eth_getLogs` address-filter list in phase 3. Must stay
/// under the RPC provider's filter-size limit.
const POOL_ADDRESS_BATCH: usize = 100;

/// Concurrent view-call fan-out for the per-contract reads we issue during
/// seeding: ERC-20 `decimals()` in phase 1 and pool `slot0()` / `liquidity()`
/// in phase 2.
const POOL_VIEW_CALL_CONCURRENCY: usize = 50;

/// Concurrency for concurrent `eth_getLogs` calls.
const LOG_FETCH_CONCURRENCY: usize = 8;

pub async fn cold_seed(
    db: &PgPool,
    network: &str,
    chain_id: u64,
    provider: AlloyProvider,
    factory: Address,
    factory_deployment_block: u64,
    snapshot_block: Option<u64>,
) -> Result<u64> {
    let snapshot_block = match snapshot_block {
        Some(b) => b,
        None => provider
            .get_block_number()
            .await
            .context("fetch current block")?,
    };

    info!(
        chain_id,
        factory_deployment_block, snapshot_block, "cold-seeding pool-indexer from chain"
    );

    let metrics = crate::metrics::Metrics::get();

    let pools = {
        let labels = [network, "discovery"];
        let _t = crate::metrics::Metrics::timer(&metrics.cold_seed_phase_seconds, &labels);
        discover_pools(&provider, factory, factory_deployment_block, snapshot_block).await?
    };
    metrics
        .cold_seed_pools_discovered
        .with_label_values(&[network])
        .set(i64::try_from(pools.len()).unwrap_or(0));
    info!(chain_id, pools = pools.len(), "pools discovered");
    persist_pools(db, chain_id, &pools).await?;

    let states = {
        let labels = [network, "state_snapshot"];
        let _t = crate::metrics::Metrics::timer(&metrics.cold_seed_phase_seconds, &labels);
        snapshot_pool_states(&provider, &pools, snapshot_block).await?
    };
    info!(chain_id, states = states.len(), "pool states snapshotted");
    persist_pool_states(db, chain_id, &states).await?;

    let active_pools: Vec<Address> = states
        .iter()
        .filter(|s| s.liquidity > 0)
        .map(|s| s.pool_address)
        .collect();
    metrics
        .cold_seed_active_pools
        .with_label_values(&[network])
        .set(i64::try_from(active_pools.len()).unwrap_or(0));
    info!(
        chain_id,
        active = active_pools.len(),
        inactive = pools.len() - active_pools.len(),
        "reconstructing ticks for active pools"
    );

    {
        let labels = [network, "tick_reconstruction"];
        let _t = crate::metrics::Metrics::timer(&metrics.cold_seed_phase_seconds, &labels);
        reconstruct_and_persist_ticks(
            db,
            chain_id,
            &provider,
            &active_pools,
            factory_deployment_block,
            snapshot_block,
        )
        .await?;
    }

    info!(chain_id, snapshot_block, "cold seeding complete");
    Ok(snapshot_block)
}

#[instrument(skip(provider))]
async fn discover_pools(
    provider: &AlloyProvider,
    factory: Address,
    from_block: u64,
    to_block: u64,
) -> Result<Vec<NewPoolData>> {
    // Chunk the full block range, fetch in parallel, decode PoolCreated events.
    let ranges: Vec<(u64, u64)> = (from_block..=to_block)
        .step_by(DISCOVERY_BLOCK_CHUNK as usize)
        .map(|start| (start, (start + DISCOVERY_BLOCK_CHUNK - 1).min(to_block)))
        .collect();

    let logs: Vec<Log> = futures::stream::iter(ranges)
        .map(|(from, to)| {
            let provider = provider.clone();
            async move { fetch_pool_created_logs(&provider, factory, from, to).await }
        })
        .buffered(LOG_FETCH_CONCURRENCY)
        .try_concat()
        .await?;

    let events: Vec<(Log, PoolCreated)> = logs
        .into_iter()
        .filter_map(|log| {
            let decoded = PoolCreated::decode_log(&log.inner).ok()?;
            Some((log, decoded.data))
        })
        .collect();

    // Fetch ERC-20 decimals for every referenced token (dedup first).
    let tokens: std::collections::HashSet<Address> = events
        .iter()
        .flat_map(|(_, e)| [e.token0, e.token1])
        .collect();
    let decimals = fetch_decimals_concurrent(provider, tokens).await;

    Ok(events
        .into_iter()
        .map(|(log, e)| NewPoolData {
            address: e.pool,
            token0: e.token0,
            token1: e.token1,
            fee: e.fee.to::<u32>(),
            token0_decimals: decimals.get(&e.token0).copied(),
            token1_decimals: decimals.get(&e.token1).copied(),
            token0_symbol: None,
            token1_symbol: None,
            created_block: log.block_number.unwrap_or(0),
        })
        .collect())
}

async fn fetch_pool_created_logs(
    provider: &AlloyProvider,
    factory: Address,
    from: u64,
    to: u64,
) -> Result<Vec<Log>> {
    bisecting_get_logs(
        provider,
        from,
        to,
        vec![factory],
        vec![PoolCreated::SIGNATURE_HASH],
    )
    .await
}

async fn fetch_decimals_concurrent(
    provider: &AlloyProvider,
    tokens: std::collections::HashSet<Address>,
) -> HashMap<Address, u8> {
    futures::stream::iter(tokens)
        .map(|token| {
            let provider = provider.clone();
            async move {
                let dec = ERC20::Instance::new(token, provider.clone())
                    .decimals()
                    .call()
                    .await
                    .ok();
                (token, dec)
            }
        })
        .buffer_unordered(POOL_VIEW_CALL_CONCURRENCY)
        .filter_map(|(token, opt)| async move { opt.map(|d| (token, d)) })
        .collect()
        .await
}

async fn persist_pools(db: &PgPool, chain_id: u64, pools: &[NewPoolData]) -> Result<()> {
    let mut tx = db.begin().await.context("begin pools tx")?;
    db::batch_insert_pools(&mut tx, chain_id, pools).await?;
    tx.commit().await.context("commit pools tx")?;
    Ok(())
}

#[instrument(skip(provider, pools))]
async fn snapshot_pool_states(
    provider: &AlloyProvider,
    pools: &[NewPoolData],
    at_block: u64,
) -> Result<Vec<PoolStateData>> {
    let addresses: Vec<Address> = pools.iter().map(|p| p.address).collect();
    let states: Vec<PoolStateData> = futures::stream::iter(addresses)
        .map(|pool| {
            let provider = provider.clone();
            async move { fetch_pool_state(&provider, pool, at_block).await }
        })
        .buffer_unordered(POOL_VIEW_CALL_CONCURRENCY)
        .filter_map(|res| async move { res })
        .collect()
        .await;
    Ok(states)
}

async fn fetch_pool_state(
    provider: &AlloyProvider,
    pool: Address,
    at_block: u64,
) -> Option<PoolStateData> {
    let instance = UniswapV3Pool::Instance::new(pool, provider.clone());
    let slot0_call = instance.slot0().block(at_block.into());
    let liquidity_call = instance.liquidity().block(at_block.into());
    let (slot0, liquidity) = tokio::join!(slot0_call.call(), liquidity_call.call());
    let slot0 = match slot0 {
        Ok(s) => s,
        Err(err) => {
            warn!(%pool, ?err, "slot0 failed");
            return None;
        }
    };
    let liquidity = match liquidity {
        Ok(l) => l,
        Err(err) => {
            warn!(%pool, ?err, "liquidity failed");
            return None;
        }
    };
    Some(PoolStateData {
        pool_address: pool,
        block_number: at_block,
        sqrt_price_x96: slot0.sqrtPriceX96,
        liquidity,
        tick: slot0.tick.as_i32(),
    })
}

async fn persist_pool_states(db: &PgPool, chain_id: u64, states: &[PoolStateData]) -> Result<()> {
    let mut tx = db.begin().await.context("begin states tx")?;
    db::batch_upsert_pool_states(&mut tx, chain_id, states).await?;
    tx.commit().await.context("commit states tx")?;
    Ok(())
}

/// Processes active pools one `POOL_ADDRESS_BATCH`-sized group at a time.
/// Each group's full history is fetched, deltas accumulated, and flushed to
/// the DB before moving on — bounds memory to roughly one batch's worth of
/// logs at any moment, and gives operators visible progress on long runs.
#[instrument(skip(db, provider, active_pools))]
async fn reconstruct_and_persist_ticks(
    db: &PgPool,
    chain_id: u64,
    provider: &AlloyProvider,
    active_pools: &[Address],
    from_block: u64,
    to_block: u64,
) -> Result<()> {
    let total = active_pools.len();
    let mut processed = 0usize;
    let mut tick_rows = 0usize;

    for pool_batch in active_pools.chunks(POOL_ADDRESS_BATCH) {
        let pool_batch = pool_batch.to_vec();
        let batch_size = pool_batch.len();

        let block_ranges: Vec<(u64, u64)> = (from_block..=to_block)
            .step_by(HISTORY_BLOCK_CHUNK as usize)
            .map(|start| (start, (start + HISTORY_BLOCK_CHUNK - 1).min(to_block)))
            .collect();

        let logs: Vec<Log> = futures::stream::iter(block_ranges)
            .map(|(from, to)| {
                let provider = provider.clone();
                let pool_batch = pool_batch.clone();
                async move { fetch_mint_burn_logs(&provider, pool_batch, from, to).await }
            })
            .buffered(LOG_FETCH_CONCURRENCY)
            .try_concat()
            .await?;

        let mut acc: HashMap<(Address, i32), i128> = HashMap::new();
        for log in logs {
            let Some(t) = log.topic0() else { continue };
            let pool = log.address();
            if *t == Mint::SIGNATURE_HASH
                && let Ok(decoded) = Mint::decode_log(&log.inner)
            {
                let e = &decoded.data;
                let amount = e.amount.cast_signed();
                *acc.entry((pool, e.tickLower.as_i32())).or_default() += amount;
                *acc.entry((pool, e.tickUpper.as_i32())).or_default() -= amount;
            } else if *t == Burn::SIGNATURE_HASH
                && let Ok(decoded) = Burn::decode_log(&log.inner)
            {
                let e = &decoded.data;
                let amount = e.amount.cast_signed();
                *acc.entry((pool, e.tickLower.as_i32())).or_default() -= amount;
                *acc.entry((pool, e.tickUpper.as_i32())).or_default() += amount;
            }
        }

        let deltas: Vec<TickDeltaData> = acc
            .into_iter()
            .filter(|(_, d)| *d != 0)
            .map(|((pool, tick), delta)| TickDeltaData {
                pool_address: pool,
                tick_idx: tick,
                delta,
            })
            .collect();

        if !deltas.is_empty() {
            db::batch_seed_ticks(db, chain_id, &deltas).await?;
            tick_rows += deltas.len();
        }

        processed += batch_size;
        info!(processed, total, tick_rows, "tick reconstruction progress");
    }
    Ok(())
}

async fn fetch_mint_burn_logs(
    provider: &AlloyProvider,
    pool_batch: Vec<Address>,
    from: u64,
    to: u64,
) -> Result<Vec<Log>> {
    let pool_count = pool_batch.len();
    bisecting_get_logs(
        provider,
        from,
        to,
        pool_batch,
        vec![Mint::SIGNATURE_HASH, Burn::SIGNATURE_HASH],
    )
    .await
    .with_context(|| format!("mint_burn_logs({from}..={to}, pools={pool_count})"))
}
