use {
    crate::{
        api::AppState,
        solver_competition::{Identifier, SolverCompetitionStoring},
    },
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        response::{IntoResponse, Json, Response},
    },
    model::{AuctionId, solver_competition::SolverCompetitionAPI},
    std::sync::Arc,
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<u64>,
) -> Response {
    if auction_id >= AuctionId::MAX.cast_unsigned() {
        return crate::solver_competition::LoadSolverCompetitionError::NotFound.into_response();
    }

    let handler: &dyn SolverCompetitionStoring = &state.database_read;
    handler
        .load_competition(Identifier::Id(auction_id.cast_signed()))
        .await
        .map(Json)
        .into_response()
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Response {
    let handler: &dyn SolverCompetitionStoring = &state.database_read;
    handler
        .load_competition(Identifier::Transaction(tx_hash))
        .await
        .map(Json)
        .into_response()
}

pub async fn get_solver_competition_latest_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SolverCompetitionAPI>, crate::solver_competition::LoadSolverCompetitionError> {
    let handler: &dyn SolverCompetitionStoring = &state.database_read;
    handler.load_latest_competition().await.map(Json)
}

#[cfg(test)]
mod tests {
    use {
        crate::solver_competition::LoadSolverCompetitionError,
        axum::response::IntoResponse,
        hyper::StatusCode,
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
