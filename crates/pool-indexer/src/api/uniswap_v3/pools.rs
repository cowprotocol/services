use {
    super::{parse_pool_ids, serialize_display, serialize_integer},
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
    std::sync::Arc,
};

/// Query parameters for the `/pools` endpoint.
///
/// Dispatch (first match wins):
/// 1. `pool_ids` — bulk lookup by pool address, returns only the requested
///    pools (no pagination). Intended for clients that already know the pool
///    addresses they care about, e.g. resolving pools referenced by an auction.
/// 2. Neither — cursor-paginated list of all pools.
#[derive(Deserialize)]
pub struct PoolsQuery {
    /// Comma-separated list of pool addresses (`0x…,0x…`). Capped at
    /// [`super::MAX_POOL_IDS_PER_REQUEST`] entries; callers with more
    /// addresses should chunk their requests.
    pub pool_ids: Option<String>,
    /// Opaque cursor returned by the previous page; omit to start from the
    /// beginning. Ignored when `pool_ids` is set.
    pub after: Option<String>,
    /// Maximum number of pools to return. Clamped to [1, 5000]; defaults to
    /// 1000. Ignored when `pool_ids` is set.
    pub limit: Option<u64>,
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
    #[serde(serialize_with = "serialize_integer")]
    pub liquidity: BigDecimal,
    #[serde(serialize_with = "serialize_integer")]
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

enum PoolsRequest<'a> {
    ByIds(&'a str),
    PaginatedList,
}

impl PoolsQuery {
    fn request(&self) -> PoolsRequest<'_> {
        if let Some(pool_ids) = self.pool_ids.as_deref() {
            PoolsRequest::ByIds(pool_ids)
        } else {
            PoolsRequest::PaginatedList
        }
    }

    fn page_limit(&self) -> u64 {
        self.limit.unwrap_or(1000).clamp(1, 5000)
    }

    fn cursor(&self) -> Result<Option<Vec<u8>>, ApiError> {
        self.after
            .as_deref()
            .map(|raw| {
                raw.parse::<Address>()
                    .map(|address| address.as_slice().to_vec())
                    .map_err(|_| ApiError::InvalidCursor)
            })
            .transpose()
    }
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

fn pools_response(
    block_number: u64,
    rows: &[db::PoolRow],
    next_cursor: Option<String>,
) -> Response {
    Json(PoolsResponse {
        block_number,
        pools: rows.iter().map(PoolResponse::from).collect(),
        next_cursor,
    })
    .into_response()
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
) -> Result<Response, ApiError> {
    let limit = query.page_limit();
    let cursor = query.cursor()?;

    // Fetch one extra row to determine if there is a next page.
    let mut rows = db::get_pools(&state.db, chain_id, cursor, limit + 1).await?;

    let has_next = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = has_next
        .then(|| rows.last().map(|row| format!("{:?}", row.address)))
        .flatten();

    Ok(pools_response(block_number, &rows, next_cursor))
}

/// Returns the pools with addresses in `pool_ids` (order not guaranteed to
/// match the request). Silently skips unknown addresses so callers can treat
/// a partial response as "these are the ones I have". Fetches the latest
/// indexed block in parallel with the pool lookup.
async fn lookup_pools_by_ids(
    state: &AppState,
    chain_id: u64,
    raw_ids: &str,
) -> Result<Response, ApiError> {
    let addresses = parse_pool_ids(raw_ids)?;
    let (block, pools) = tokio::join!(
        latest_indexed_block(state, chain_id),
        db::get_pools_by_ids(&state.db, chain_id, &addresses),
    );
    Ok(pools_response(block?, &pools?, None))
}

/// `GET /api/v1/{network}/uniswap/v3/pools`
///
/// Dispatches based on query params — see [`PoolsQuery`].
pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(query): Query<PoolsQuery>,
) -> Result<Response, ApiError> {
    let chain_id = resolve_chain_id(&state, &network)?;

    match query.request() {
        PoolsRequest::ByIds(pool_ids) => lookup_pools_by_ids(&state, chain_id, pool_ids).await,
        PoolsRequest::PaginatedList => {
            let block_number = latest_indexed_block(&state, chain_id).await?;
            list_pools(&state, chain_id, block_number, &query).await
        }
    }
}
