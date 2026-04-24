use {
    super::{PoolIds, serialize_integer},
    crate::{
        api::{ApiError, AppState, latest_indexed_block, resolve_chain_id},
        db::uniswap_v3 as db,
    },
    alloy_primitives::Address,
    axum::{
        extract::{Path, Query, State},
        response::{IntoResponse, Json, Response},
    },
    bigdecimal::BigDecimal,
    serde::{Deserialize, Serialize},
    std::{collections::HashMap, sync::Arc},
};

/// A single tick entry with its net liquidity.
#[derive(Serialize)]
pub struct TickEntry {
    pub tick_idx: i32,
    #[serde(serialize_with = "serialize_integer")]
    pub liquidity_net: BigDecimal,
}

impl From<db::TickRow> for TickEntry {
    fn from(tick: db::TickRow) -> Self {
        Self {
            tick_idx: tick.tick_idx,
            liquidity_net: tick.liquidity_net,
        }
    }
}

#[derive(Serialize)]
pub struct TicksResponse {
    pub block_number: u64,
    pub pool: Address,
    pub ticks: Vec<TickEntry>,
}

/// Query parameters for the bulk ticks endpoint.
#[derive(Deserialize)]
pub struct BulkTicksQuery {
    /// Comma-separated list of pool addresses (`0x…,0x…`) parsed eagerly.
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

pub async fn get_ticks(
    State(state): State<Arc<AppState>>,
    Path((network, pool)): Path<(String, Address)>,
) -> Result<Response, ApiError> {
    let chain_id = resolve_chain_id(&state, &network)?;

    let (block, ticks) = tokio::join!(
        latest_indexed_block(&state, chain_id),
        db::get_ticks(&state.db, chain_id, &pool),
    );

    Ok(Json(TicksResponse {
        block_number: block?,
        pool,
        ticks: ticks?.into_iter().map(TickEntry::from).collect(),
    })
    .into_response())
}

/// `GET /api/v1/{network}/uniswap/v3/pools/ticks?pool_ids=0x…,0x…`
///
/// Bulk tick fetch for many pools in one round trip. Replaces the subgraph's
/// `TICKS_BY_POOL_IDS_QUERY`. Ticks are grouped by pool and sorted by
/// `tick_idx` within each group. Per-pool tick count is bounded by the DB
/// helper (see [`db::MAX_TICKS_PER_POOL`]).
pub async fn get_ticks_bulk(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(BulkTicksQuery { pool_ids }): Query<BulkTicksQuery>,
) -> Result<Response, ApiError> {
    let chain_id = resolve_chain_id(&state, &network)?;

    let (block, ticks) = tokio::join!(
        latest_indexed_block(&state, chain_id),
        db::get_ticks_for_pools(&state.db, chain_id, &pool_ids.0),
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
