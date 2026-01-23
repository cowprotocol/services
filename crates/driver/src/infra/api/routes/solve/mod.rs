pub mod dto;

pub use dto::AuctionError;
use {
    crate::{
        domain::competition,
        infra::{
            api::{Error, State},
            observe,
        },
    },
    axum::{RequestExt, body::Body, http::Request},
    hyper::body::Bytes,
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
    let handle_request = async {
        let body_bytes = collect_request_body(request).await?;
        let competition = state.competition();
        let result = competition.solve(body_bytes).await;
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

async fn collect_request_body(request: Request<Body>) -> Result<Bytes, competition::Error> {
    tracing::debug!("received request");
    let start = std::time::Instant::now();

    let body = request.into_limited_body().map_err(|err| {
        tracing::warn!(?err, "request body exceeds size limit");
        competition::Error::MalformedRequest
    })?;
    let body_bytes = hyper::body::to_bytes(body).await.map_err(|err| {
        tracing::warn!(?err, "failed to stream request body");
        competition::Error::MalformedRequest
    })?;

    tracing::debug!(time = ?start.elapsed(), "finished streaming body");
    Ok(body_bytes)
}
