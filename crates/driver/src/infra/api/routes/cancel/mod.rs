mod dto;

use {
    crate::{
        domain::competition::auction,
        infra::api::{self, Error, State, extract::LoggingJson},
    },
    tracing::Instrument,
};

pub(in crate::infra::api) fn cancel(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/cancel", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    LoggingJson(req): LoggingJson<dto::CancelRequest>,
) -> Result<axum::Json<dto::CancelResponse>, (axum::http::StatusCode, axum::Json<Error>)> {
    let auction_id =
        auction::Id::try_from(req.auction_id).map_err(api::routes::AuctionError::from)?;
    let solver = state.solver().name().to_string();

    async move {
        let withdrawal = state.competition().withdraw(auction_id, req.solution_id);
        Ok(axum::Json(withdrawal.into()))
    }
    .instrument(tracing::info_span!("/cancel", solver, %auction_id))
    .await
}
