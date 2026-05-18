//! HTTP routing for the pool-indexer API. Keeps the wiring — route table,
//! middleware, span extraction — separate from the type definitions in
//! `super` so either side can change without churn in the other.

use {
    super::{AppState, uniswap_v3},
    axum::{
        Router,
        extract::{MatchedPath, Request},
        http::StatusCode,
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::get,
    },
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::sync::Arc,
    tower::ServiceBuilder,
    tower_http::trace::TraceLayer,
};

/// Builds the full axum `Router` for the pool-indexer API. Mounts handlers,
/// attaches the metrics middleware, and wires the distributed-tracing layer
/// so `traceparent` / B3 headers on incoming requests seed the current
/// span — letting logs correlate across services.
pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/{network}/uniswap/v3/pools",
            get(uniswap_v3::get_pools),
        )
        .route(
            "/api/v1/{network}/uniswap/v3/pools/by-ids",
            get(uniswap_v3::get_pools_by_ids),
        )
        .route(
            "/api/v1/{network}/uniswap/v3/pools/ticks",
            get(uniswap_v3::get_ticks_bulk),
        )
        .route(
            "/api/v1/{network}/uniswap/v3/pools/{pool_address}/ticks",
            get(uniswap_v3::get_ticks),
        )
        .with_state(state)
        .layer(middleware::from_fn(record_request_metrics))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().make_span_with(make_span))
                .map_request(record_trace_id),
        )
}

async fn health() -> impl IntoResponse {
    StatusCode::OK
}

/// Emits per-request `api_requests` (count) and `api_request_seconds`
/// (latency) metrics labelled by the matched route template (e.g.
/// `/api/v1/{network}/uniswap/v3/pools`) rather than the concrete URL — so
/// the cardinality stays bounded no matter how many networks / addresses
/// flow through.
async fn record_request_metrics(req: Request, next: Next) -> Response {
    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let metrics = crate::metrics::Metrics::get();
    let labels = [route.as_str()];
    let _timer = crate::metrics::Metrics::timer(&metrics.api_request_seconds, &labels);
    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();
    metrics
        .api_requests
        .with_label_values(&[route.as_str(), status.as_str()])
        .inc();
    response
}
