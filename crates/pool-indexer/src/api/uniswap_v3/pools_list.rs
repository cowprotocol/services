//! `GET /api/v1/{network}/uniswap/v3/pools` — cursor-paginated listing of
//! all indexed pools.

use {
    super::pools_response,
    crate::{
        api::{ApiError, AppState, ensure_network_configured, latest_indexed_block},
        db::uniswap_v3 as db,
    },
    alloy_primitives::Address,
    axum::{
        extract::{Path, Query, State},
        response::Response,
    },
    serde::Deserialize,
    std::sync::Arc,
};

/// Default number of pools to return per page when the client doesn't
/// specify a `limit`.
const DEFAULT_PAGE_LIMIT: u64 = 1_000;

/// Hard cap on `limit` to bound both query time and response size. Server
/// applies this even if the client asks for more.
const MAX_PAGE_LIMIT: u64 = 5_000;

/// Query parameters for the `GET /pools` endpoint.
#[derive(Deserialize)]
pub struct ListPoolsQuery {
    /// Opaque cursor returned by the previous page; omit to start from the
    /// beginning.
    pub after: Option<String>,
    /// Maximum number of pools to return. Clamped to `[1, MAX_PAGE_LIMIT]`;
    /// defaults to `DEFAULT_PAGE_LIMIT`.
    pub limit: Option<u64>,
}

impl ListPoolsQuery {
    /// Resolve the effective page size.
    fn page_limit(&self) -> u64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_LIMIT)
            .clamp(1, MAX_PAGE_LIMIT)
    }

    /// Parse the opaque `after` cursor back to the 20-byte address key used
    /// by the DB's keyset pagination. Returns `InvalidCursor` on malformed
    /// input so caller sees a 400 rather than an empty page.
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

/// Returns a cursor-paginated list of all indexed pools, ordered by address.
pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(query): Query<ListPoolsQuery>,
) -> Result<Response, ApiError> {
    ensure_network_configured(&state, &network)?;
    let block_number = latest_indexed_block(&state).await?;
    let limit = query.page_limit();
    let cursor = query.cursor()?;

    let mut rows = db::get_pools(&state.db, cursor, limit + 1).await?;

    let has_next = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = has_next
        .then(|| rows.last().map(|row| format!("{:#x}", row.address)))
        .flatten();

    Ok(pools_response(block_number, &rows, next_cursor))
}
