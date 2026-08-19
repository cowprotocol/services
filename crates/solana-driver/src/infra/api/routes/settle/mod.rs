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
///
/// On-chain settlement will be implemented in follow-up PRs; until then
/// this route will panic.
pub async fn settle(
    state: axum::extract::State<State>,
    Json(request): Json<dto::SettleRequest>,
) -> Result<Json<dto::SettleResponse>, (StatusCode, Json<Error>)> {
    let auction_id = auction::Id::try_from(request.auction_id)?;
    let solution_id = request.solution_id;

    let handle_request = async {
        state.competition().settle(auction_id, solution_id)?;
        unreachable!("competition.settle panics until on-chain settlement is implemented")
    };

    handle_request
        .instrument(tracing::info_span!(
            "/settle",
            auction_id = %auction_id,
            solution_id
        ))
        .await
}
