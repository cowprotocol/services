use {
    super::{bad_request, internal_error, parse_hex_address, parse_pool_ids, serialize_display},
    crate::{api::AppState, db::uniswap_v3 as db},
    alloy_primitives::Address,
    axum::{
        extract::{Path, Query, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    bigdecimal::BigDecimal,
    serde::{Deserialize, Serialize},
    std::sync::Arc,
};

/// Query parameters for the `/pools` endpoint.
///
/// Dispatch (first match wins):
/// 1. `pool_ids` — bulk lookup by pool address, returns only the requested
///    pools (no pagination). Intended for clients that already know the pool
///    addresses they care about, e.g. resolving pools referenced by an auction.
/// 2. `token0` (+ optional `token1`) — symbol search. Returns all matching
///    pools, ordered by liquidity descending. No pagination.
/// 3. Neither — cursor-paginated list of all pools.
#[derive(Deserialize)]
pub struct PoolsQuery {
    /// Comma-separated list of pool addresses (`0x…,0x…`). Capped at
    /// [`super::MAX_POOL_IDS_PER_REQUEST`] entries; callers with more
    /// addresses should chunk their requests.
    pub pool_ids: Option<String>,
    /// Opaque cursor returned by the previous page; omit to start from the
    /// beginning. Ignored when `pool_ids` or `token0` is set.
    pub after: Option<String>,
    /// Maximum number of pools to return. Clamped to [1, 5000]; defaults to
    /// 1000. Ignored when `pool_ids` or `token0` is set.
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
    pub id: Address,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
}

/// A single Uniswap v3 pool.
#[derive(Serialize)]
pub struct PoolResponse {
    /// Checksummed pool contract address.
    pub id: Address,
    pub token0: TokenInfo,
    pub token1: TokenInfo,
    /// Fee tier in hundredths of a basis point (e.g. 3000 = 0.3%).
    #[serde(serialize_with = "serialize_display")]
    pub fee_tier: u32,
    #[serde(serialize_with = "serialize_display")]
    pub liquidity: BigDecimal,
    #[serde(serialize_with = "serialize_display")]
    pub sqrt_price: BigDecimal,
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
    /// Cursor to pass as `after` to fetch the next page; `null` on the last
    /// page.
    pub next_cursor: Option<String>,
}

impl From<&db::PoolRow> for PoolResponse {
    fn from(r: &db::PoolRow) -> Self {
        Self {
            id: r.address,
            token0: TokenInfo {
                id: r.token0,
                decimals: r.token0_decimals,
                symbol: non_empty(&r.token0_symbol),
            },
            token1: TokenInfo {
                id: r.token1,
                decimals: r.token1_decimals,
                symbol: non_empty(&r.token1_symbol),
            },
            fee_tier: r.fee,
            liquidity: r.liquidity.clone(),
            sqrt_price: r.sqrt_price_x96.clone(),
            tick: r.tick,
            ticks: None,
        }
    }
}

/// Empty strings are a "tried-and-failed" sentinel written by the symbol
/// backfill task; surface them as missing rather than as `""`.
fn non_empty(s: &Option<String>) -> Option<String> {
    s.as_ref().filter(|s| !s.is_empty()).cloned()
}

/// Returns all pools whose token symbols match the given filter(s).
/// When only `token0` is supplied, matches any pool containing that symbol.
/// When both are supplied, both symbols must match (order-independent).
/// Results are ordered by liquidity descending; no pagination is applied.
async fn search_pools(
    state: &AppState,
    chain_id: u64,
    block_number: u64,
    token0: &str,
    token1: Option<&str>,
) -> Response {
    let rows = if let Some(token1) = token1 {
        db::search_pools_by_pair(&state.db, chain_id, token0, token1).await
    } else {
        db::search_pools_by_token(&state.db, chain_id, token0).await
    };
    match rows {
        Ok(rows) => Json(PoolsResponse {
            block_number,
            pools: rows.iter().map(PoolResponse::from).collect(),
            next_cursor: None,
        })
        .into_response(),
        Err(err) => internal_error(err),
    }
}

/// Returns a cursor-paginated list of all indexed pools, ordered by address.
/// Fetches `limit + 1` rows to detect whether a next page exists; the extra
/// row is stripped from the response and its address is returned as
/// `next_cursor`.
async fn list_pools(
    state: &AppState,
    chain_id: u64,
    block_number: u64,
    query: &PoolsQuery,
) -> Response {
    let limit = query.limit.unwrap_or(1000).clamp(1, 5000);

    let cursor_bytes = match query.after.as_deref().map(parse_hex_address) {
        Some(Ok(addr)) => Some(addr.as_slice().to_vec()),
        Some(Err(_)) => return bad_request("invalid cursor"),
        None => None,
    };

    // Fetch one extra row to determine if there is a next page.
    let rows = match cursor_bytes {
        Some(cursor) => db::get_pools_after(&state.db, chain_id, cursor, limit + 1).await,
        None => db::get_pools(&state.db, chain_id, limit + 1).await,
    };
    let rows = match rows {
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

    Json(PoolsResponse {
        block_number,
        pools: rows.iter().map(PoolResponse::from).collect(),
        next_cursor,
    })
    .into_response()
}

/// Returns the pools with addresses in `pool_ids` (order not guaranteed to
/// match the request). Silently skips unknown addresses so callers can treat
/// a partial response as "these are the ones I have". Fetches the latest
/// indexed block in parallel with the pool lookup.
async fn lookup_pools_by_ids(state: &AppState, chain_id: u64, raw_ids: &str) -> Response {
    let addresses = match parse_pool_ids(raw_ids) {
        Ok(a) => a,
        Err(resp) => return resp,
    };
    let (block_res, pools_res) = tokio::join!(
        db::get_latest_indexed_block(&state.db, chain_id),
        db::get_pools_by_ids(&state.db, chain_id, &addresses),
    );
    let block_number = match block_res {
        Ok(Some(block)) => block,
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(err) => return internal_error(err),
    };
    match pools_res {
        Ok(rows) => Json(PoolsResponse {
            block_number,
            pools: rows.iter().map(PoolResponse::from).collect(),
            next_cursor: None,
        })
        .into_response(),
        Err(err) => internal_error(err),
    }
}

/// `GET /api/v1/{network}/uniswap/v3/pools`
///
/// Dispatches based on query params — see [`PoolsQuery`].
pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(query): Query<PoolsQuery>,
) -> Response {
    let chain_id = match state.resolve_network(&network) {
        Some(id) => id,
        None => return StatusCode::NOT_FOUND.into_response(),
    };

    if let Some(pool_ids) = query.pool_ids.as_deref() {
        return lookup_pools_by_ids(&state, chain_id, pool_ids).await;
    }

    let block_number = match db::get_latest_indexed_block(&state.db, chain_id).await {
        Ok(Some(block)) => block,
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(err) => return internal_error(err),
    };

    if let Some(token0) = query.token0.as_deref() {
        return search_pools(
            &state,
            chain_id,
            block_number,
            token0,
            query.token1.as_deref(),
        )
        .await;
    }
    list_pools(&state, chain_id, block_number, &query).await
}
