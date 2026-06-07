//! `GET /api/v1/{network}/uniswap/v3/pools/by-ids?pool_ids=…` — bulk lookup
//! of specific pool addresses, no pagination.

use {
    super::{PoolIds, pools_response},
    crate::{
        api::{ApiError, AppState, ensure_network_configured, latest_indexed_block},
        db::uniswap_v3 as db,
    },
    axum::{
        extract::{Path, Query, State},
        response::Response,
    },
    serde::Deserialize,
    std::sync::Arc,
};

/// Query parameters for the bulk lookup endpoint. Intended for clients that
/// already know the pool addresses they care about, e.g. resolving pools
/// referenced by an auction.
#[derive(Deserialize)]
pub struct BulkLookupQuery {
    /// Comma-separated list of pool addresses capped at
    /// [`super::MAX_POOL_IDS_PER_REQUEST`] entries. Callers
    /// with more addresses should chunk their requests.
    pub pool_ids: PoolIds,
}

/// Returns the pools with addresses in `pool_ids` (order not guaranteed to
/// match the request). Silently skips unknown addresses so callers can treat
/// a partial response as "these are the ones I have". `block_number` is read
/// sequentially before the rows so the envelope is never newer than the data.
pub async fn get_pools_by_ids(
    State(state): State<Arc<AppState>>,
    Path(network): Path<String>,
    Query(BulkLookupQuery { pool_ids }): Query<BulkLookupQuery>,
) -> Result<Response, ApiError> {
    ensure_network_configured(&state, &network)?;
    let block = latest_indexed_block(&state).await?;
    let pools = db::get_pools_by_ids(&state.db, &pool_ids.0).await?;
    Ok(pools_response(block, &pools, None))
}
