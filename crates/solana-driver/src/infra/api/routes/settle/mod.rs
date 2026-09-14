pub mod dto;

use {
    crate::{
        domain::auction,
        infra::api::{LoggingJson, State, error::Error},
    },
    axum::{Json, http::StatusCode},
    tracing::Instrument,
};

/// Handle `POST /settle`: validate the request, then submit the solution and
/// wait for confirmation.
pub(crate) async fn settle(
    state: axum::extract::State<State>,
    LoggingJson(request): LoggingJson<dto::SettleRequest>,
) -> Result<Json<dto::SettleResponse>, (StatusCode, Json<Error>)> {
    let auction_id = auction::Id::try_from(request.auction_id)?;
    state
        .competition()
        .settle(
            auction_id,
            request.solution_id,
            request.submission_deadline_slot,
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
