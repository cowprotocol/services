pub mod dto;

use {
    crate::{
        domain::auction,
        infra::api::{
            LoggingJson,
            State,
            error::{Error, Kind},
        },
    },
    axum::{Json, http::StatusCode},
    solana_sdk::transaction::VersionedTransaction,
    tracing::Instrument,
};

/// Handle `POST /settle`: validate the request, land the sponsored creation
/// transactions, then submit the solution and wait for confirmation.
pub(crate) async fn settle(
    state: axum::extract::State<State>,
    LoggingJson(request): LoggingJson<dto::SettleRequest>,
) -> Result<Json<dto::SettleResponse>, (StatusCode, Json<Error>)> {
    let auction_id = auction::Id::try_from(request.auction_id)?;
    let creations: Vec<VersionedTransaction> = request
        .creations
        .iter()
        .map(|bytes| bincode::deserialize(bytes))
        .collect::<Result<_, _>>()
        .map_err(|_| <(StatusCode, Json<Error>)>::from(Kind::InvalidCreation))?;
    state
        .competition()
        .settle(
            auction_id,
            request.solution_id,
            request.submission_deadline_slot,
            creations,
        )
        .instrument(tracing::info_span!(
            "/settle",
            solver = %state.competition().solver_name(),
            auction_id = %auction_id,
            solution_id = request.solution_id,
        ))
        .await
        .map(dto::SettleResponse::new)
        .map(Json)
        .map_err(Into::into)
}
