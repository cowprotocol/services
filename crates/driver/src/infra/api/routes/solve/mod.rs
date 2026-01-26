pub mod dto;

pub use dto::AuctionError;
use {
    crate::{
        domain::competition,
        infra::{
            api::{Error, REQUEST_BODY_LIMIT, State},
            observe,
        },
    },
    axum::{body::Body, http::Request},
    hyper::body::Bytes,
    std::time::{Duration, Instant},
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
        let body_bytes = collect_request_body(request, solver).await?;
        let competition = state.competition();
        let result = competition.solve(body_bytes).await;
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

async fn collect_request_body(
    request: Request<Body>,
    solver: &str,
) -> Result<Bytes, competition::Error> {
    tracing::trace!("start streaming request body");
    let start = Instant::now();

    // accepting the raw request bypasses axum's request body limiting layer
    // so we have to manually ensure the body has a reasonable size.
    let limited_body = http_body::Limited::new(request.into_body(), REQUEST_BODY_LIMIT);
    let body_bytes = hyper::body::to_bytes(limited_body).await.map_err(|err| {
        tracing::warn!(?err, "failed to stream request body");
        competition::Error::MalformedRequest
    })?;

    let duration = start.elapsed();
    Metrics::measure_solve_transfer_time(solver, duration);
    tracing::trace!(?duration, "finished streaming request body");
    Ok(body_bytes)
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Time spent by the driver reading the full solve request body into
    /// memory.
    #[metric(labels("solver"))]
    #[metric(buckets(0.0001, 0.0005, 0.002, 0.05, 0.1, 0.2, 0.3, 0.4, 0.5, 0.75, 1, 1.5))]
    solve_request_body_read_duration_seconds: prometheus::HistogramVec,
}

impl Metrics {
    fn get() -> &'static Metrics {
        Metrics::instance(::observe::metrics::get_storage_registry())
            .expect("unexpected error getting metrics instance")
    }

    fn measure_solve_transfer_time(solver: &str, time: Duration) {
        Self::get()
            .solve_request_body_read_duration_seconds
            .with_label_values(&[solver])
            .observe(time.as_secs_f64());
    }
}
