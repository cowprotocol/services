use {
    crate::{api::AppState, solver_competition::LoadSolverCompetitionError},
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::{AuctionId, solver_competition_v2::Response as CompetitionResponse},
    std::{str::FromStr, sync::Arc},
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(auction_id) = auction_id.parse::<AuctionId>() else {
        return StatusCode::NOT_FOUND.into_response();
    };

    state
        .database_read
        .load_competition_by_id(auction_id)
        .await
        .map(Json)
        .into_response()
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<String>,
) -> Response {
    // TODO: remove after all downstream callers have been notified of the status
    // code changes
    let Ok(tx_hash) = B256::from_str(&tx_hash) else {
        return StatusCode::NOT_FOUND.into_response();
    };

    state
        .database_read
        .load_competition_by_tx_hash(tx_hash)
        .await
        .map(Json)
        .into_response()
}

pub async fn get_solver_competition_latest_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CompetitionResponse>, LoadSolverCompetitionError> {
    state
        .database_read
        .load_latest_competition()
        .await
        .map(Json)
}
