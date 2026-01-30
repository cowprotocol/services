use {
    crate::{
        api::AppState,
        solver_competition::{Identifier, LoadSolverCompetitionError, SolverCompetitionStoring},
    },
    alloy::primitives::B256,
    axum::{
        extract::{Path, State},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
    },
    model::{AuctionId, solver_competition::SolverCompetitionAPI},
    std::sync::Arc,
};

pub async fn get_solver_competition_by_id_handler(
    State(state): State<Arc<AppState>>,
    Path(auction_id): Path<AuctionId>,
) -> Response {
    let handler: &dyn SolverCompetitionStoring = &state.database_read;
    let result = handler.load_competition(Identifier::Id(auction_id)).await;
    response(result)
}

pub async fn get_solver_competition_by_hash_handler(
    State(state): State<Arc<AppState>>,
    Path(tx_hash): Path<B256>,
) -> Response {
    let handler: &dyn SolverCompetitionStoring = &state.database_read;
    let result = handler
        .load_competition(Identifier::Transaction(tx_hash))
        .await;
    response(result)
}

pub async fn get_solver_competition_latest_handler(State(state): State<Arc<AppState>>) -> Response {
    let handler: &dyn SolverCompetitionStoring = &state.database_read;
    let result = handler.load_latest_competition().await;
    response(result)
}

fn response(
    result: Result<SolverCompetitionAPI, crate::solver_competition::LoadSolverCompetitionError>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_response_ok() {
        let result: Result<SolverCompetitionAPI, LoadSolverCompetitionError> =
            Ok(Default::default());
        let resp = response(result);
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_response_not_found() {
        let result: Result<SolverCompetitionAPI, LoadSolverCompetitionError> =
            Err(LoadSolverCompetitionError::NotFound);
        let resp = response(result);
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }
}
