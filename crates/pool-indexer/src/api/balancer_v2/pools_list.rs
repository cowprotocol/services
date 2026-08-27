//! `GET /api/v1/{network}/balancer/v2/pools` — cursor-paginated pool list.

use {
    super::pools_response,
    crate::{
        api::{ApiError, AppState, latest_indexed_block},
        db::balancer_v2 as db,
    },
    alloy_primitives::B256,
    axum::{
        extract::{Query, State},
        response::Response,
    },
    serde::Deserialize,
    std::sync::Arc,
};

const DEFAULT_PAGE_LIMIT: u64 = 1_000;

/// Hard server-side cap on `limit`. Applied even if the client asks for more.
const MAX_PAGE_LIMIT: u64 = 5_000;

#[derive(Deserialize)]
pub struct ListPoolsQuery {
    /// Cursor from the previous page (the last-seen `pool_id`); omit to start
    /// from the beginning.
    pub after: Option<B256>,
    /// Clamped to `[1, MAX_PAGE_LIMIT]`; defaults to `DEFAULT_PAGE_LIMIT`.
    pub limit: Option<u64>,
}

impl ListPoolsQuery {
    fn page_limit(&self) -> u64 {
        self.limit
            .unwrap_or(DEFAULT_PAGE_LIMIT)
            .clamp(1, MAX_PAGE_LIMIT)
    }
}

/// All indexed pools, sorted by `pool_id`.
pub async fn get_pools(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListPoolsQuery>,
) -> Result<Response, ApiError> {
    let block_number = latest_indexed_block(&state.db, &state.balancer_v2_factories).await?;
    let limit = query.page_limit();
    let cursor = query.after.map(|id| id.as_slice().to_vec());

    let mut rows = db::get_pools(&state.db, cursor, limit + 1).await?;

    let has_next = rows.len() > limit as usize;
    rows.truncate(limit as usize);
    let next_cursor = has_next
        .then(|| rows.last().map(|row| format!("{:#x}", row.pool_id)))
        .flatten();

    Ok(pools_response(block_number, rows, next_cursor))
}
