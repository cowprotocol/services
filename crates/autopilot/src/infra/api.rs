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
    std::{
        net::SocketAddr,
        ops::RangeInclusive,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::oneshot,
};

/// Minimum allowed timeout for price estimation requests.
/// Values below this are not useful as they don't give estimators enough time.
const MIN_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone)]
struct State {
    estimator: Arc<dyn NativePriceEstimating>,
    allowed_timeout: RangeInclusive<Duration>,
}

#[derive(Debug, Deserialize)]
struct NativePriceQuery {
    /// Optional timeout in milliseconds for the price estimation request.
    /// If not provided, uses the default timeout configured for autopilot.
    /// Values below 250ms are automatically clamped to the minimum (250ms).
    /// Values exceeding the configured maximum are clamped to the maximum.
    #[serde(default)]
    timeout_ms: Option<u64>,
}

pub async fn serve(
    addr: SocketAddr,
    estimator: Arc<dyn NativePriceEstimating>,
    max_timeout: Duration,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), std::io::Error> {
    let state = State {
        estimator,
        allowed_timeout: MIN_TIMEOUT..=max_timeout,
    };

    let app = Router::new()
        .route("/native_price/{token}", get(get_native_price))
        .with_state(state)
        // Layers are applied as a stack (last applied = outermost)
        .layer(axum::middleware::from_fn(
            |req, next: axum::middleware::Next| async move {
                let req = record_trace_id(req);
                next.run(req).await
            },
        ))
        .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span));

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!(?addr, "serving HTTP API");

    axum::serve(listener, app)
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
    let timeout = query
        .timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(*state.allowed_timeout.end())
        .clamp(*state.allowed_timeout.start(), *state.allowed_timeout.end());

    let start = Instant::now();
    match state.estimator.estimate_native_price(token, timeout).await {
        Ok(price) => {
            let elapsed = start.elapsed();
            tracing::debug!(
                ?token,
                ?timeout,
                ?elapsed,
                ?price,
                "estimated native token price"
            );
            Json(NativeTokenPrice { price }).into_response()
        }
        Err(err) => {
            let elapsed = start.elapsed();
            tracing::warn!(
                ?err,
                ?token,
                ?timeout,
                ?elapsed,
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
