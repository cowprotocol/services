use {
    crate::{
        api::AppState,
        solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    },
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        response::Json,
    },
    model::{AuctionId, solver_competition::SolverCompetitionAPI},
    std::sync::Arc,
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<u64>,
) -> Result<Json<SolverCompetitionAPI>, LoadSolverCompetitionError> {
    // We use u64 to ensure that negative numbers are returned as BAD_REQUEST
    // however, there's a gap between u64::MAX and i64::MAX, numbers beyond i64::MAX
    // will be marked as NOT_FOUND as they're positive (and as such, valid) but
    // they are not covered by our system
    if auction_id >= AuctionId::MAX.cast_unsigned() {
        return Err(LoadSolverCompetitionError::NotFound);
    }

    db(&state)
        .load_competition(
            Identifier::Id(auction_id.cast_signed()),
            state.hide_competition_before_block(),
        )
        .await
        .map(Json)
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Result<Json<SolverCompetitionAPI>, LoadSolverCompetitionError> {
    db(&state)
        .load_competition(
            Identifier::Transaction(tx_hash),
            state.hide_competition_before_block(),
        )
        .await
        .map(Json)
}

pub async fn get_solver_competition_latest_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SolverCompetitionAPI>, LoadSolverCompetitionError> {
    SolverCompetitionStoring::load_latest_competition(
        db(&state),
        state.hide_competition_before_block(),
    )
    .await
    .map(Json)
}

fn db(state: &AppState) -> &dyn SolverCompetitionStoring {
    // While these queries actually don't write to the DB
    // the latency incurred by the DB replication process
    // is not acceptable in some cases (e.g. when the circuit
    // breaker needs to decide whether an tx was out of competition).
    &state.database_write
}

#[cfg(test)]
mod tests {
    use {
        crate::solver_competition::LoadSolverCompetitionError,
        axum::{http::StatusCode, response::IntoResponse},
    };

    #[tokio::test]
    async fn test_response_not_found() {
        let error = LoadSolverCompetitionError::NotFound;
        let resp = error.into_response();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_response_internal_error() {
        let error = LoadSolverCompetitionError::Other(anyhow::anyhow!("test error"));
        let resp = error.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
