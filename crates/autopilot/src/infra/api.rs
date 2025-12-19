use {
    alloy::primitives::Address,
    axum::{
        Router,
        extract::{Path, Query, State as AxumState},
        http::StatusCode,
        response::{IntoResponse, Json, Response},
        routing::get,
    },
    model::quote::NativeTokenPrice,
    observe::distributed_tracing::tracing_axum::{make_span, record_trace_id},
    serde::Deserialize,
    shared::price_estimation::{PriceEstimationError, native::NativePriceEstimating},
    std::{net::SocketAddr, sync::Arc, time::Duration},
    tokio::sync::oneshot,
};

#[derive(Clone)]
struct State {
    estimator: Arc<dyn NativePriceEstimating>,
    timeout: Duration,
}

#[derive(Debug, Deserialize)]
struct NativePriceQuery {
    /// Optional timeout in milliseconds for the price estimation request.
    /// If not provided, uses the default timeout configured for autopilot.
    #[serde(default)]
    timeout_ms: Option<u64>,
}

pub async fn serve(
    addr: SocketAddr,
    estimator: Arc<dyn NativePriceEstimating>,
    timeout: Duration,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), hyper::Error> {
    let state = State { estimator, timeout };

    let app = Router::new()
        .route("/native_price/:token", get(get_native_price))
        .with_state(state)
        .layer(
            tower::ServiceBuilder::new()
                .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span))
                .map_request(record_trace_id),
        );

    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    tracing::info!(?addr, "serving HTTP API");

    server
        .with_graceful_shutdown(async {
            shutdown.await.ok();
        })
        .await
}

async fn get_native_price(
    Path(token): Path<Address>,
    Query(query): Query<NativePriceQuery>,
    AxumState(state): AxumState<State>,
) -> Response {
    let timeout = match query.timeout_ms {
        Some(0) => {
            return (StatusCode::BAD_REQUEST, "timeout_ms must be greater than 0").into_response();
        }
        Some(requested_ms) => {
            let requested = Duration::from_millis(requested_ms);
            if requested > state.timeout {
                return (
                    StatusCode::BAD_REQUEST,
                    format!(
                        "timeout_ms cannot exceed configured maximum of {}ms",
                        state.timeout.as_millis()
                    ),
                )
                    .into_response();
            }
            requested
        }
        None => state.timeout,
    };

    match state.estimator.estimate_native_price(token, timeout).await {
        Ok(price) => {
            tracing::debug!(?token, ?timeout, ?price, "estimated native token price");
            Json(NativeTokenPrice { price }).into_response()
        }
        Err(err) => {
            tracing::warn!(
                ?err,
                ?token,
                ?timeout,
                "failed to estimate native token price"
            );
            error_to_response(err)
        }
    }
}

fn error_to_response(err: PriceEstimationError) -> Response {
    match err {
        PriceEstimationError::NoLiquidity | PriceEstimationError::EstimatorInternal(_) => {
            (StatusCode::NOT_FOUND, "No liquidity").into_response()
        }
        PriceEstimationError::UnsupportedToken { token: _, reason } => (
            StatusCode::BAD_REQUEST,
            format!("Unsupported token, reason: {reason}"),
        )
            .into_response(),
        PriceEstimationError::RateLimited => {
            (StatusCode::TOO_MANY_REQUESTS, "Rate limited").into_response()
        }
        PriceEstimationError::UnsupportedOrderType(reason) => (
            StatusCode::BAD_REQUEST,
            format!("Unsupported order type, reason: {reason}"),
        )
            .into_response(),
        PriceEstimationError::ProtocolInternal(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}
