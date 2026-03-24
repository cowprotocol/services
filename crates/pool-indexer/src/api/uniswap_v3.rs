use {
    crate::{api::AppState, db::uniswap_v3 as db},
    alloy_primitives::Address,
    axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    serde::{Deserialize, Serialize},
    std::sync::Arc,
};

fn internal_error(err: anyhow::Error) -> Response {
    tracing::error!(?err, "internal error");
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

// ── /api/v1/uniswap/v3/pools ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct PoolsQuery {
    pub after: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct TokenInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

#[derive(Serialize)]
pub struct PoolResponse {
    pub id: String,
    pub token0: TokenInfo,
    pub token1: TokenInfo,
    pub fee_tier: String,
    pub liquidity: String,
    pub sqrt_price: String,
    pub tick: i32,
    pub ticks: Option<Vec<TickEntry>>,
}

#[derive(Serialize)]
pub struct PoolsResponse {
    pub block_number: u64,
    pub pools: Vec<PoolResponse>,
    pub next_cursor: Option<String>,
}

pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PoolsQuery>,
) -> Response {
    let block_number = match db::get_latest_indexed_block(&state.db, state.chain_id).await {
        Ok(Some(block)) => block,
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(err) => return internal_error(err),
    };

    let limit = query.limit.unwrap_or(1000).clamp(1, 5000);

    let cursor_bytes = match query.after.as_deref().map(parse_hex_address) {
        Some(Ok(addr)) => Some(addr.as_slice().to_vec()),
        Some(Err(_)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid cursor"})),
            )
                .into_response();
        }
        None => None,
    };

    // Fetch one extra row to determine if there is a next page.
    let rows = match db::get_pools(&state.db, state.chain_id, cursor_bytes, limit + 1).await {
        Ok(rows) => rows,
        Err(err) => return internal_error(err),
    };

    let limit_usize = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_next = rows.len() > limit_usize;
    let rows = if has_next {
        &rows[..limit_usize]
    } else {
        &rows[..]
    };

    let next_cursor = if has_next {
        rows.last().map(|r| format!("{:?}", r.address))
    } else {
        None
    };

    let pools = rows
        .iter()
        .map(|r| PoolResponse {
            id: format!("{:?}", r.address),
            token0: TokenInfo {
                id: format!("{:?}", r.token0),
                decimals: r.token0_decimals,
                symbol: r.token0_symbol.clone(),
            },
            token1: TokenInfo {
                id: format!("{:?}", r.token1),
                decimals: r.token1_decimals,
                symbol: r.token1_symbol.clone(),
            },
            fee_tier: r.fee.to_string(),
            liquidity: r.liquidity.to_string(),
            sqrt_price: r.sqrt_price_x96.to_string(),
            tick: r.tick,
            ticks: None,
        })
        .collect();

    Json(PoolsResponse {
        block_number,
        pools,
        next_cursor,
    })
    .into_response()
}

// ── /api/v1/uniswap/v3/pools/:pool_address/ticks ────────────────────────────

#[derive(Serialize)]
pub struct TickEntry {
    pub tick_idx: i32,
    pub liquidity_net: String,
}

#[derive(Serialize)]
pub struct TicksResponse {
    pub block_number: u64,
    pub pool: String,
    pub ticks: Vec<TickEntry>,
}

pub async fn get_ticks(
    State(state): State<Arc<AppState>>,
    Path(pool_address): Path<String>,
) -> Response {
    let addr = match parse_hex_address(&pool_address) {
        Ok(a) => a,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid pool address"})),
            )
                .into_response();
        }
    };

    let block_number = match db::get_latest_indexed_block(&state.db, state.chain_id).await {
        Ok(Some(block)) => block,
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(err) => return internal_error(err),
    };

    let ticks = match db::get_ticks(&state.db, state.chain_id, &addr).await {
        Ok(ticks) => ticks,
        Err(err) => return internal_error(err),
    };

    Json(TicksResponse {
        block_number,
        pool: format!("{:?}", addr),
        ticks: ticks
            .into_iter()
            .map(|t| TickEntry {
                tick_idx: t.tick_idx,
                liquidity_net: t.liquidity_net.to_string(),
            })
            .collect(),
    })
    .into_response()
}

fn parse_hex_address(s: &str) -> Result<Address, &'static str> {
    s.parse::<Address>().map_err(|_| "invalid address")
}
