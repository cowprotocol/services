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
    if auction_id >= AuctionId::MAX.cast_unsigned() {
        return Err(LoadSolverCompetitionError::NotFound);
    }
    let c = db(&state)
        .load_competition(Identifier::Id(auction_id.cast_signed()))
        .await?;
    filter_by_deadline(&state, c).await.map(Json)
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Result<Json<SolverCompetitionAPI>, LoadSolverCompetitionError> {
    let c = db(&state)
        .load_competition(Identifier::Transaction(tx_hash))
        .await?;
    filter_by_deadline(&state, c).await.map(Json)
}

pub async fn get_solver_competition_latest_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SolverCompetitionAPI>, LoadSolverCompetitionError> {
    let c = db(&state).load_latest_competition().await?;
    filter_by_deadline(&state, c).await.map(Json)
}

async fn filter_by_deadline(
    state: &AppState,
    competition: SolverCompetitionAPI,
) -> Result<SolverCompetitionAPI, LoadSolverCompetitionError> {
    if state.hide_competition_before_deadline
        && let Some(deadline) = state
            .database_write
            .get_auction_deadline(competition.auction_id)
            .await?
        && !state.is_competition_visible(deadline)
    {
        return Err(LoadSolverCompetitionError::NotFound);
    }
    Ok(competition)
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
