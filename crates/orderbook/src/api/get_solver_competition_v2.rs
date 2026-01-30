use {
    crate::{api::AppState, solver_competition::LoadSolverCompetitionError},
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::{AuctionId, solver_competition_v2},
    std::sync::Arc,
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<AuctionId>,
) -> Response {
    let result = state.database_read.load_competition_by_id(auction_id).await;
    response(result)
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Response {
    let result = state
        .database_read
        .load_competition_by_tx_hash(tx_hash)
        .await;
    response(result)
}

pub async fn get_solver_competition_latest_handler(State(state): State<Arc<AppState>>) -> Response {
    let result = state.database_read.load_latest_competition().await;
    response(result)
}

fn response(
    result: Result<
        solver_competition_v2::Response,
        crate::solver_competition::LoadSolverCompetitionError,
    >,
) -> Response {
    match result {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(LoadSolverCompetitionError::NotFound) => (
            StatusCode::NOT_FOUND,
            super::error("NotFound", "no competition found"),
        )
            .into_response(),
        Err(LoadSolverCompetitionError::Other(err)) => {
            tracing::error!(?err, "load solver competition");
            crate::api::internal_error_reply()
        }
    }
}
