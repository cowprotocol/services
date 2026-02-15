pub mod dto;

pub use dto::AuctionError;
use {
    crate::infra::{
        api::{Error, State},
        observe,
    },
    axum::{body::Body, http::Request},
    tracing::Instrument,
};

pub(in crate::infra::api) fn solve(router: axum::Router<State>) -> axum::Router<State> {
    router.route("/solve", axum::routing::post(route))
}

async fn route(
    state: axum::extract::State<State>,
    // Take the request as raw request to extract the body as a stream.
    // This delays interpreting the data as much as possible and allows
    // logging how long the raw data transfer takes.
    request: Request<Body>,
) -> Result<axum::Json<dto::SolveResponse>, (hyper::StatusCode, axum::Json<Error>)> {
    let solver = state.solver().name().as_str();

    let handle_request = async {
        let competition = state.competition();
        let result = competition.solve(request).await;
        // Solving takes some time, so there is a chance for the settlement queue to
        // have capacity again.
        competition.ensure_settle_queue_capacity()?;
        observe::solved(solver, &result);
        Ok(axum::Json(dto::SolveResponse::new(
            result?,
            &competition.solver,
        )))
    };

    handle_request
        .instrument(tracing::info_span!(
            "/solve",
            solver,
            auction_id = tracing::field::Empty
        ))
        .await
}
