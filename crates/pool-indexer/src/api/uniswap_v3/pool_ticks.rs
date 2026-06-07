//! `GET /api/v1/{network}/uniswap/v3/pools/{pool}/ticks` — every non-zero
//! tick for a single pool.

use {
    super::TickEntry,
    crate::{
        api::{ApiError, AppState, ensure_network_configured, latest_indexed_block},
        db::uniswap_v3 as db,
    },
    alloy_primitives::Address,
    axum::{
        extract::{Path, State},
        response::{IntoResponse, Json, Response},
    },
    serde::Serialize,
    std::sync::Arc,
};

#[derive(Serialize)]
pub struct TicksResponse {
    pub block_number: u64,
    pub pool: Address,
    pub ticks: Vec<TickEntry>,
}

/// Returns all non-zero ticks for one pool, ordered by `tick_idx`.
pub async fn get_ticks(
    State(state): State<Arc<AppState>>,
    Path((network, pool)): Path<(String, Address)>,
) -> Result<Response, ApiError> {
    ensure_network_configured(&state, &network)?;

    let (block, ticks) = tokio::join!(
        latest_indexed_block(&state),
        db::get_ticks(&state.db, &pool),
    );

    Ok(Json(TicksResponse {
        block_number: block?,
        pool,
        ticks: ticks?.into_iter().map(TickEntry::from).collect(),
    })
    .into_response())
}
