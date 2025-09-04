pub mod dto;

pub use dto::AuctionError;
use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    std::sync::Arc,
    tracing::Instrument,
};

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    // take the request body as a raw string to delay parsing as much
    // as possible because many requests don't have to be parsed at all
    req: String,
) -> Result<axum::Json<dto::SolveResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    let handle_request = async {
        let competition = state.competition();
        let result = competition.solve(Arc::new(req)).await;
        // Solving takes some time, so there is a chance for the settlement queue to
        // have capacity again.
        competition.ensure_settle_queue_capacity()?;
        observe::solved(state.solver().name(), &result);
        Ok(axum::Json(dto::SolveResponse::new(
            result?,
            &competition.solver,
        )))
    };

    handle_request
        .instrument(tracing::info_span!("/solve", solver = %state.solver().name(), auction_id = tracing::field::Empty))
        .await
}
