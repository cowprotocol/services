use {
    super::{PoolIds, serialize_display, serialize_integer},
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

/// Query parameters for the `GET /pools` endpoint — cursor-paginated full
/// listing.
#[derive(Deserialize)]
pub struct ListPoolsQuery {
    /// Opaque cursor returned by the previous page; omit to start from the
    /// beginning.
    pub after: Option<String>,
    /// Maximum number of pools to return. Clamped to [1, 5000]; defaults to
    /// 1000.
    pub limit: Option<u64>,
}

/// Query parameters for the `GET /pools/by-ids` endpoint — bulk lookup of
/// specific pool addresses, returns only the requested pools (no pagination).
/// Intended for clients that already know the pool addresses they care about,
/// e.g. resolving pools referenced by an auction.
#[derive(Deserialize)]
pub struct BulkLookupQuery {
    /// Comma-separated list of pool addresses (`0x…,0x…`) parsed eagerly.
    /// Capped at [`super::MAX_POOL_IDS_PER_REQUEST`] entries; callers with
    /// more addresses should chunk their requests.
    pub pool_ids: PoolIds,
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

/// Default number of pools to return per page when the client doesn't
/// specify a `limit`. Sized so a full mainnet pool set can be drained in
/// a few pages.
const DEFAULT_PAGE_LIMIT: u64 = 1_000;

/// Hard cap on `limit` to bound both query time and response size. Server
/// applies this even if the client asks for more.
const MAX_PAGE_LIMIT: u64 = 5_000;

impl ListPoolsQuery {
    /// Resolve the effective page size: the client-supplied `limit` clamped
    /// to `[1, MAX_PAGE_LIMIT]`, defaulting to `DEFAULT_PAGE_LIMIT`.
    fn page_limit(&self) -> u64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_LIMIT)
            .clamp(1, MAX_PAGE_LIMIT)
    }

    /// Parse the opaque `after` cursor back to the 20-byte address key used
    /// by the DB's keyset pagination. Returns `InvalidCursor` on malformed
    /// input so callers see a 400 rather than an empty page.
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

/// Converts a slice of DB rows into the on-the-wire [`PoolsResponse`]
/// envelope, attaching the indexed-block tag and optional pagination
/// cursor. Centralised here so every route emits the same JSON shape.
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

/// `GET /api/v1/{network}/uniswap/v3/pools`
///
/// Returns a cursor-paginated list of all indexed pools, ordered by address.
///
/// Pagination is last-value-seen: the DB query returns `limit + 1` rows to
/// detect whether a next page exists, the extra row is dropped, and the
/// address of the last row in the returned page becomes the `next_cursor`.
/// The next request passes that back as `after=…`, and the DB uses
/// `WHERE address > $cursor` to pick up from the row immediately after it —
/// so the cursor points at the *last row served*, not the next one to
/// serve.
pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(query): Query<ListPoolsQuery>,
) -> Result<Response, ApiError> {
    let chain_id = resolve_chain_id(&state, &network)?;
    let block_number = latest_indexed_block(&state, chain_id).await?;
    let limit = query.page_limit();
    let cursor = query.cursor()?;

    let mut rows = db::get_pools(&state.db, chain_id, cursor, limit + 1).await?;

    let has_next = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = has_next
        .then(|| rows.last().map(|row| format!("{:#x}", row.address)))
        .flatten();

    Ok(pools_response(block_number, &rows, next_cursor))
}

/// `GET /api/v1/{network}/uniswap/v3/pools/by-ids?pool_ids=0x…,0x…`
///
/// Returns the pools with addresses in `pool_ids` (order not guaranteed to
/// match the request). Silently skips unknown addresses so callers can treat
/// a partial response as "these are the ones I have". Fetches the latest
/// indexed block in parallel with the pool lookup.
pub async fn get_pools_by_ids(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(BulkLookupQuery { pool_ids }): Query<BulkLookupQuery>,
) -> Result<Response, ApiError> {
    let chain_id = resolve_chain_id(&state, &network)?;
    let (block, pools) = tokio::join!(
        latest_indexed_block(&state, chain_id),
        db::get_pools_by_ids(&state.db, chain_id, &pool_ids.0),
    );
    Ok(pools_response(block?, &pools?, None))
}
