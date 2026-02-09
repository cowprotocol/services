use {
    crate::{api::AppState, solver_competition::LoadSolverCompetitionError},
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        response::Json,
    },
    model::{AuctionId, solver_competition_v2::Response as CompetitionResponse},
    std::sync::Arc,
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<AuctionId>,
) -> Result<Json<CompetitionResponse>, LoadSolverCompetitionError> {
    state
        .database_read
        .load_competition_by_id(auction_id)
        .await
        .map(Json)
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Result<Json<CompetitionResponse>, LoadSolverCompetitionError> {
    state
        .database_read
        .load_competition_by_tx_hash(tx_hash)
        .await
        .map(Json)
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
