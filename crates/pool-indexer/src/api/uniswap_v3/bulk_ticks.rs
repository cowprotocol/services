//! `GET /api/v1/{network}/uniswap/v3/pools/ticks?pool_ids=…`

use {
    super::{PoolIds, TickEntry},
    crate::{
        api::{ApiError, AppState, latest_indexed_block},
        db::uniswap_v3 as db,
    },
    alloy_primitives::Address,
    axum::{
        extract::{Query, State},
        response::{IntoResponse, Json, Response},
    },
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
};

#[derive(Deserialize)]
pub struct BulkTicksQuery {
    /// Comma-separated pool addresses. Capped at
    /// [`super::MAX_POOL_IDS_PER_REQUEST`].
    pub pool_ids: PoolIds,
}

#[derive(Serialize)]
pub struct PoolTicks {
    pub pool: Address,
    pub ticks: Vec<TickEntry>,
}

/// Pools with no non-zero ticks are omitted from `pools` — callers should
/// treat an address missing from the response as "no active ticks", not
/// "unknown pool".
#[derive(Serialize)]
pub struct BulkTicksResponse {
    pub block_number: u64,
    pub pools: Vec<PoolTicks>,
}

/// Ticks per pool are sorted by `tick_idx` (the DB query orders by
/// `(pool_address, tick_idx)`).
pub async fn get_ticks_bulk(
    State(state): State<Arc<AppState>>,
    Query(BulkTicksQuery { pool_ids }): Query<BulkTicksQuery>,
) -> Result<Response, ApiError> {
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
