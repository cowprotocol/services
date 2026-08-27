//! `GET /api/v1/{network}/balancer/v2/pools/by-ids?pool_ids=…`

use {
    super::{PoolIds, pools_response},
    crate::{
        api::{ApiError, AppState, latest_indexed_block},
        db::balancer_v2 as db,
    },
    axum::{
        extract::{Query, State},
        response::Response,
    },
    serde::Deserialize,
    std::sync::Arc,
};

#[derive(Deserialize)]
pub struct BulkLookupQuery {
    /// Comma-separated 32-byte pool ids. Capped at
    /// [`super::MAX_POOL_IDS_PER_REQUEST`]; clients with more should chunk.
    pub pool_ids: PoolIds,
}

/// Pools matching `pool_ids`, sorted by `pool_id`. Unknown ids are skipped
/// silently; treat a partial response as "these are the ones I have".
///
/// `block_number` is read first so the envelope is never *newer* than the row
/// data (the indexer can advance between the two reads, never regress).
pub async fn get_pools_by_ids(
    State(state): State<Arc<AppState>>,
    Query(BulkLookupQuery { pool_ids }): Query<BulkLookupQuery>,
) -> Result<Response, ApiError> {
    let block = latest_indexed_block(&state.db, &state.balancer_v2_factories).await?;
    let pools = db::get_pools_by_ids(&state.db, &pool_ids.0).await?;
    Ok(pools_response(block, pools, None))
}
