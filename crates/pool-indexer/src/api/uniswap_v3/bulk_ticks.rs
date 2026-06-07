//! `GET /api/v1/{network}/uniswap/v3/pools/ticks?pool_ids=…` — bulk tick
//! fetch for many pools in one round trip.

use {
    super::{PoolIds, TickEntry},
    crate::{
        api::{ApiError, AppState, ensure_network_configured, latest_indexed_block},
        db::uniswap_v3 as db,
    },
    alloy_primitives::Address,
    axum::{
        extract::{Path, Query, State},
        response::{IntoResponse, Json, Response},
    },
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
};

/// Query parameters for the bulk ticks endpoint.
#[derive(Deserialize)]
pub struct BulkTicksQuery {
    /// Comma-separated list of pool addresses
    /// Capped at [`super::MAX_POOL_IDS_PER_REQUEST`] entries.
    pub pool_ids: PoolIds,
}

/// One pool's worth of ticks in a bulk response.
#[derive(Serialize)]
pub struct PoolTicks {
    pub pool: Address,
    pub ticks: Vec<TickEntry>,
}

/// Envelope for `GET /pools/ticks`. Only pools with at least one non-zero
/// tick appear in `pools` — callers resolving many addresses at once should
/// treat a missing pool as "no active ticks" rather than "unknown pool".
#[derive(Serialize)]
pub struct BulkTicksResponse {
    pub block_number: u64,
    pub pools: Vec<PoolTicks>,
}

/// Bulk tick fetch for many pools in one round trip. Replaces the subgraph's
/// `TICKS_BY_POOL_IDS_QUERY`. Ticks are grouped by pool and sorted by
/// `tick_idx` within each group. No per-pool cap is applied; callers limit
/// via [`super::MAX_POOL_IDS_PER_REQUEST`].
pub async fn get_ticks_bulk(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(BulkTicksQuery { pool_ids }): Query<BulkTicksQuery>,
) -> Result<Response, ApiError> {
    ensure_network_configured(&state, &network)?;

    let (block, ticks) = tokio::join!(
        latest_indexed_block(&state),
        db::get_ticks_for_pools(&state.db, &pool_ids.0),
    );

    Ok(Json(BulkTicksResponse {
        block_number: block?,
        pools: group_ticks_by_pool(ticks?),
    })
    .into_response())
}

/// Bucket the flat row stream (one entry per `(pool, tick)`) into one
/// [`PoolTicks`] per pool. The DB query orders rows by `(pool_address,
/// tick_idx)`, so the resulting per-pool tick vectors come out sorted.
fn group_ticks_by_pool(rows: Vec<db::PoolTickRow>) -> Vec<PoolTicks> {
    let mut groups: HashMap<Address, Vec<TickEntry>> = HashMap::with_capacity(rows.len());
    for row in rows {
        groups.entry(row.pool_address).or_default().push(TickEntry {
            tick_idx: row.tick_idx,
            liquidity_net: row.liquidity_net,
        });
    }
    groups
        .into_iter()
        .map(|(pool, ticks)| PoolTicks { pool, ticks })
        .collect()
}
