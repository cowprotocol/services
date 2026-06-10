//! HTTP routing for the pool-indexer API. Keeps the wiring — route table,
//! middleware, span extraction — separate from the type definitions in
//! `super` so either side can change without churn in the other.

use {
    super::{ApiError, AppState, uniswap_v3},
    axum::{
        Router,
        extract::{MatchedPath, Path, Request, State},
        http::StatusCode,
        middleware::{self, Next},
        response::{IntoResponse, Response},
        routing::get,
    },
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::{collections::HashMap, sync::Arc},
    tower::ServiceBuilder,
    tower_http::trace::TraceLayer,
};

/// Builds the axum `Router` for the pool-indexer API, and mounts
/// the routes and the metrics middleware.
pub fn router(state: Arc<AppState>) -> Router {
    let v3_routes = Router::new()
        .route("/pools", get(uniswap_v3::get_pools))
        .route("/pools/by-ids", get(uniswap_v3::get_pools_by_ids))
        .route("/pools/ticks", get(uniswap_v3::get_ticks_bulk))
        .route("/pools/{pool_address}/ticks", get(uniswap_v3::get_ticks))
        .route_layer(middleware::from_fn_with_state(state.clone(), network_guard));

    Router::new()
        .route("/health", get(health))
        .nest("/api/v1/{network}/uniswap/v3", v3_routes)
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

/// Rejects requests whose `{network}` path segment doesn't match the
/// network this process is configured for. Centralised here so every
/// network-scoped handler is gated by one check instead of each one
/// re-running the same guard.
async fn network_guard(
    State(state): State<Arc<AppState>>,
    Path(params): Path<HashMap<String, String>>,
    req: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let network = params.get("network").ok_or(ApiError::NetworkNotFound)?;
    if !state.is_network_configured(network) {
        return Err(ApiError::NetworkNotFound);
    }
    Ok(next.run(req).await)
}

/// Emits per-request `api_requests` (count) and `api_request_seconds`
/// (latency) metrics labelled by the matched route template (e.g.
/// `/api/v1/{network}/uniswap/v3/pools`) rather than the concrete URL — so
/// the cardinality stays bounded regardless of how many pool addresses
/// flow through the address-parameterised routes.
async fn record_request_metrics(req: Request, next: Next) -> Response {
    use crate::metrics::HistogramVecExt;

    let route = req
        .extensions()
        .get::<MatchedPath>()
        .map(|p| p.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".to_owned());
    let metrics = crate::metrics::Metrics::get();
    let labels = [route.as_str()];
    let _timer = metrics.api_request_seconds.timer(&labels);
    let response = next.run(req).await;
    let status = response.status().as_u16().to_string();
    metrics
        .api_requests
        .with_label_values(&[route.as_str(), status.as_str()])
        .inc();
    response
}
