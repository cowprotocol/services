use {
    crate::{api::AppState, db::uniswap_v3 as db},
    axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    serde::{Deserialize, Serialize},
    std::sync::Arc,
};

use super::{internal_error, parse_hex_address};

/// Query parameters for the `/pools` endpoint.
///
/// If `token0` is provided the response contains only matching pools (no
/// pagination). If both `token0` and `token1` are provided the search is
/// narrowed to that exact pair. Without any token filter the endpoint returns
/// a cursor-paginated list of all pools.
#[derive(Deserialize)]
pub struct PoolsQuery {
    /// Opaque cursor returned by the previous page; omit to start from the beginning.
    pub after: Option<String>,
    /// Maximum number of pools to return. Clamped to [1, 5000]; defaults to 1000.
    pub limit: Option<i64>,
    /// Filter by token symbol (partial, case-insensitive). Acts as the "base"
    /// token when `token1` is also supplied. Matched via SQL `LIKE` against
    /// the stored symbol — use a symbol fragment (e.g. `"WETH"`, `"USD"`),
    /// not a contract address.
    pub token0: Option<String>,
    /// Paired with `token0` to filter by an exact token pair (both symbols
    /// must match, order-independent).
    pub token1: Option<String>,
}

/// ERC-20 token metadata embedded in pool responses.
#[derive(Serialize)]
pub struct TokenInfo {
    /// Checksummed contract address.
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// A single Uniswap v3 pool.
#[derive(Serialize)]
pub struct PoolResponse {
    /// Checksummed pool contract address.
    pub id: String,
    pub token0: TokenInfo,
    pub token1: TokenInfo,
    /// Fee tier in hundredths of a basis point (e.g. 3000 = 0.3%).
    pub fee_tier: String,
    pub liquidity: String,
    pub sqrt_price: String,
    pub tick: i32,
    /// Populated only when tick data is explicitly requested.
    pub ticks: Option<Vec<super::ticks::TickEntry>>,
}

/// Response envelope for pool listing and search endpoints.
#[derive(Serialize)]
pub struct PoolsResponse {
    /// Latest block that has been fully indexed.
    pub block_number: u64,
    pub pools: Vec<PoolResponse>,
    /// Cursor to pass as `after` to fetch the next page; `null` on the last page.
    pub next_cursor: Option<String>,
}

fn pool_row_to_response(r: &db::PoolRow) -> PoolResponse {
    PoolResponse {
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
    }
}

/// Returns all pools whose token symbols match the given filter(s).
/// When only `token0` is supplied, matches any pool containing that symbol.
/// When both are supplied, both symbols must match (order-independent).
/// Results are ordered by liquidity descending; no pagination is applied.
async fn search_pools(
    state: &AppState,
    block_number: u64,
    token0: &str,
    token1: Option<&str>,
) -> Response {
    let rows = if let Some(token1) = token1 {
        db::search_pools_by_pair(&state.db, state.chain_id, token0, token1).await
    } else {
        db::search_pools_by_token(&state.db, state.chain_id, token0).await
    };
    match rows {
        Ok(rows) => Json(PoolsResponse {
            block_number,
            pools: rows.iter().map(pool_row_to_response).collect(),
            next_cursor: None,
        })
        .into_response(),
        Err(err) => internal_error(err),
    }
}

/// Returns a cursor-paginated list of all indexed pools, ordered by address.
/// Fetches `limit + 1` rows to detect whether a next page exists; the extra
/// row is stripped from the response and its address is returned as `next_cursor`.
async fn list_pools(state: &AppState, block_number: u64, query: &PoolsQuery) -> Response {
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
    let rows = match cursor_bytes {
        Some(cursor) => db::get_pools_after(&state.db, state.chain_id, cursor, limit + 1).await,
        None => db::get_pools(&state.db, state.chain_id, limit + 1).await,
    };
    let rows = match rows {
        Ok(rows) => rows,
        Err(err) => return internal_error(err),
    };

    let limit_usize = usize::try_from(limit).unwrap_or(usize::MAX);
    let has_next = rows.len() > limit_usize;
    let rows = if has_next { &rows[..limit_usize] } else { &rows[..] };
    let next_cursor = if has_next {
        rows.last().map(|r| format!("{:?}", r.address))
    } else {
        None
    };

    Json(PoolsResponse {
        block_number,
        pools: rows.iter().map(pool_row_to_response).collect(),
        next_cursor,
    })
    .into_response()
}

/// `GET /api/v1/{network}/uniswap/v3/pools`
///
/// Dispatches to [`search_pools`] when a token filter is present, or
/// [`list_pools`] for paginated listing of all pools.
pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(query): Query<PoolsQuery>,
) -> Response {
    if network != state.network_name {
        return StatusCode::NOT_FOUND.into_response();
    }
    let block_number = match db::get_latest_indexed_block(&state.db, state.chain_id).await {
        Ok(Some(block)) => block,
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(err) => return internal_error(err),
    };

    if let Some(token0) = query.token0.as_deref() {
        return search_pools(&state, block_number, token0, query.token1.as_deref()).await;
    }
    list_pools(&state, block_number, &query).await
}
