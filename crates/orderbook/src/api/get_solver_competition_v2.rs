use {
    crate::{api::AppState, database::Postgres, solver_competition::LoadSolverCompetitionError},
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        response::{IntoResponse, Json, Response},
    },
    model::{AuctionId, solver_competition_v2::Response as CompetitionResponse},
    std::sync::Arc,
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<u64>,
) -> Response {
    // We use u64 to ensure that negative numbers are returned as BAD_REQUEST
    // however, there's a gap between u64::MAX and i64::MAX, numbers beyond i64::MAX
    // will be marked as NOT_FOUND as they're positive (and as such, valid) but
    // they are not covered by our system
    if auction_id > AuctionId::MAX.cast_unsigned() {
        return LoadSolverCompetitionError::NotFound.into_response();
    }

    db(&state)
        .load_competition_by_id(auction_id.cast_signed())
        .await
        .and_then(|r| filter_by_deadline(&state, r))
        .map(Json)
        .into_response()
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Response {
    db(&state)
        .load_competition_by_tx_hash(tx_hash)
        .await
        .and_then(|r| filter_by_deadline(&state, r))
        .map(Json)
        .into_response()
}

pub async fn get_solver_competition_latest_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<CompetitionResponse>, LoadSolverCompetitionError> {
    let response = db(&state).load_latest_competition().await?;
    filter_by_deadline(&state, response).map(Json)
}

fn filter_by_deadline(
    state: &AppState,
    response: CompetitionResponse,
) -> Result<CompetitionResponse, LoadSolverCompetitionError> {
    if !state.is_competition_visible(response.auction_deadline_block) {
        return Err(LoadSolverCompetitionError::NotFound);
    }
    Ok(response)
}

fn db(state: &AppState) -> &Postgres {
    // While these queries actually don't write to the DB
    // the latency incurred by the DB replication process
    // is not acceptable in some cases (e.g. when the circuit
    // breaker needs to decide whether an tx was out of competition).
    &state.database_write
}
