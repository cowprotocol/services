pub mod dto;

use {
    crate::{
        domain::auction,
        infra::api::{State, error::Error},
    },
    axum::{Json, http::StatusCode},
    tracing::Instrument,
};

/// Handle `POST /settle`: validate the request, then submit the solution.
pub async fn settle(
    state: axum::extract::State<State>,
    Json(request): Json<dto::SettleRequest>,
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
            auction_id = %auction_id,
            solution_id = request.solution_id,
        ))
        .await
        .map(dto::SettleResponse::new)
        .map(Json)
        .map_err(|error| {
            tracing::warn!(?error, "settle failed");
            error.into()
        })
}
