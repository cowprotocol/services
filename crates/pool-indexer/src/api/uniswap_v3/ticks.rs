use {
    crate::{api::AppState, db::uniswap_v3 as db},
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    serde::Serialize,
    std::sync::Arc,
};

use super::{internal_error, parse_hex_address};

/// A single tick entry with its net liquidity.
#[derive(Serialize)]
pub struct TickEntry {
    pub tick_idx: i32,
    pub liquidity_net: String,
}

#[derive(Serialize)]
pub struct TicksResponse {
    pub block_number: u64,
    pub pool: String,
    pub ticks: Vec<TickEntry>,
}

pub async fn get_ticks(
    State(state): State<Arc<AppState>>,
    Path((network, pool_address)): Path<(String, String)>,
) -> Response {
    if network != state.network_name {
        return StatusCode::NOT_FOUND.into_response();
    }
    let addr = match parse_hex_address(&pool_address) {
        Ok(a) => a,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid pool address"})),
            )
                .into_response();
        }
    };

    let block_number = match db::get_latest_indexed_block(&state.db, state.chain_id).await {
        Ok(Some(block)) => block,
        Ok(None) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        Err(err) => return internal_error(err),
    };

    let ticks = match db::get_ticks(&state.db, state.chain_id, &addr).await {
        Ok(ticks) => ticks,
        Err(err) => return internal_error(err),
    };

    Json(TicksResponse {
        block_number,
        pool: format!("{:?}", addr),
        ticks: ticks
            .into_iter()
            .map(|t| TickEntry {
                tick_idx: t.tick_idx,
                liquidity_net: t.liquidity_net.to_string(),
            })
            .collect(),
    })
    .into_response()
}
