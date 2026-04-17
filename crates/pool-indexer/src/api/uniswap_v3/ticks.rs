use {
    super::{bad_request, internal_error, parse_hex_address, parse_pool_ids, serialize_display},
    crate::{
        api::{AppState, latest_indexed_block, resolve_chain_id},
        db::uniswap_v3 as db,
    },
    alloy_primitives::Address,
    axum::{
        extract::{Path, Query, State},
        response::{IntoResponse, Json, Response},
    },
    bigdecimal::BigDecimal,
    serde::{Deserialize, Serialize},
    std::sync::Arc,
};

/// A single tick entry with its net liquidity.
#[derive(Serialize)]
pub struct TickEntry {
    pub tick_idx: i32,
    #[serde(serialize_with = "serialize_display")]
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
    /// Comma-separated list of pool addresses (`0x…,0x…`). Capped at
    /// [`super::MAX_POOL_IDS_PER_REQUEST`] entries.
    pub pool_ids: String,
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
    Path((network, pool_address)): Path<(String, String)>,
) -> Response {
    let chain_id = match resolve_chain_id(&state, &network) {
        Ok(chain_id) => chain_id,
        Err(response) => return response,
    };
    let pool = match parse_hex_address(&pool_address) {
        Ok(pool) => pool,
        Err(_) => return bad_request("invalid pool address"),
    };

    let (block_result, ticks_result) = tokio::join!(
        latest_indexed_block(&state, chain_id),
        db::get_ticks(&state.db, chain_id, &pool),
    );
    let block_number = match block_result {
        Ok(block_number) => block_number,
        Err(response) => return response,
    };
    let ticks = match ticks_result {
        Ok(rows) => rows,
        Err(err) => return internal_error(err),
    };

    Json(TicksResponse {
        block_number,
        pool,
        ticks: ticks.into_iter().map(TickEntry::from).collect(),
    })
    .into_response()
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
    Query(query): Query<BulkTicksQuery>,
) -> Response {
    let chain_id = match resolve_chain_id(&state, &network) {
        Ok(chain_id) => chain_id,
        Err(response) => return response,
    };

    let pool_ids = match parse_pool_ids(&query.pool_ids) {
        Ok(pool_ids) => pool_ids,
        Err(response) => return response,
    };

    let (block_result, ticks_result) = tokio::join!(
        latest_indexed_block(&state, chain_id),
        db::get_ticks_for_pools(&state.db, chain_id, &pool_ids),
    );
    let block_number = match block_result {
        Ok(block_number) => block_number,
        Err(response) => return response,
    };
    let rows = match ticks_result {
        Ok(rows) => rows,
        Err(err) => return internal_error(err),
    };

    Json(BulkTicksResponse {
        block_number,
        pools: group_ticks_by_pool(rows),
    })
    .into_response()
}

fn group_ticks_by_pool(rows: Vec<db::PoolTickRow>) -> Vec<PoolTicks> {
    let mut pools: Vec<PoolTicks> = Vec::new();

    for row in rows {
        let tick = TickEntry {
            tick_idx: row.tick_idx,
            liquidity_net: row.liquidity_net,
        };

        match pools.last_mut() {
            Some(last) if last.pool == row.pool_address => last.ticks.push(tick),
            _ => pools.push(PoolTicks {
                pool: row.pool_address,
                ticks: vec![tick],
            }),
        }
    }

    pools
}
