//! `GET /api/v1/{network}/uniswap/v3/pools/{pool}/ticks` — every non-zero
//! tick for a single pool.

use {
    super::TickEntry,
    crate::{
        api::{ApiError, AppState, latest_indexed_block, resolve_chain_id},
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
    let chain_id = resolve_chain_id(&state, &network)?;

    let (block, ticks) = tokio::join!(
        latest_indexed_block(&state, chain_id),
        db::get_ticks(&state.db, chain_id, &pool),
    );

    Ok(Json(TicksResponse {
        block_number: block?,
        pool,
        ticks: ticks?.into_iter().map(TickEntry::from).collect(),
    })
    .into_response())
}
