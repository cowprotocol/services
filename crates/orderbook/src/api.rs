use {
    crate::{app_data, database::Postgres, orderbook::Orderbook, quoter::QuoteHandler},
    axum::{
        Router,
        extract::DefaultBodyLimit,
        http::{Request, StatusCode},
        middleware::{self, Next},
        response::{IntoResponse, Json, Response},
    },
    observe::distributed_tracing::tracing_axum,
    serde::{Deserialize, Serialize},
    shared::price_estimation::{PriceEstimationError, native::NativePriceEstimating},
    std::{
        borrow::Cow,
        fmt::Debug,
        sync::Arc,
        time::{Duration, Instant},
    },
    tower_http::{cors::CorsLayer, trace::TraceLayer},
};

mod cancel_order;
mod cancel_orders;
mod get_app_data;
mod get_auction;
mod get_native_price;
mod get_order_by_uid;
mod get_order_status;
mod get_orders_by_tx;
mod get_solver_competition;
mod get_solver_competition_v2;
mod get_token_metadata;
mod get_total_surplus;
mod get_trades;
mod get_trades_v2;
mod get_user_orders;
mod post_order;
mod post_quote;
mod put_app_data;
mod version;

/// Centralized application state shared across all API handlers
#[derive(Clone)]
pub struct AppState {
    pub database_write: Postgres,
    pub database_read: Postgres,
    pub orderbook: Arc<Orderbook>,
    pub quotes: Arc<QuoteHandler>,
    pub app_data: Arc<app_data::Registry>,
    pub native_price_estimator: Arc<dyn NativePriceEstimating>,
    pub quote_timeout: Duration,
}

/// Middleware that automatically tracks metrics using Axum's MatchedPath
async fn with_matched_path_metric(
    req: Request<axum::body::Body>,
    next: Next<axum::body::Body>,
) -> Response {
    let metrics = ApiMetrics::instance(observe::metrics::get_storage_registry()).unwrap();

    // Extract matched path and HTTP method
    let method = req.method().as_str();
    let matched_path = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str())
        .unwrap_or("unknown");

    // Create label in format "METHOD /path"
    let label = format!("{} {}", method, matched_path);

    let timer = Instant::now();
    let response = next.run(req).await;
    let status = response.status();

    // Track completed requests
    metrics.on_request_completed(&label, status, timer);

    // Track rejected requests (4xx and 5xx status codes)
    if status.is_client_error() || status.is_server_error() {
        metrics
            .requests_rejected
            .with_label_values(&[status.as_str()])
            .inc();
    }

    response
}

const MAX_JSON_BODY_PAYLOAD: u64 = 1024 * 16;

pub fn handle_all_routes(
    database_write: Postgres,
    database_read: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    app_data: Arc<app_data::Registry>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    quote_timeout: Duration,
) -> Router {
    // Capture app_data size limit before moving it into state
    let app_data_size_limit = app_data.size_limit();

    let state = Arc::new(AppState {
        database_write,
        database_read,
        orderbook,
        quotes,
        app_data,
        native_price_estimator,
        quote_timeout,
    });

    // Initialize metrics
    let metrics = ApiMetrics::instance(observe::metrics::get_storage_registry()).unwrap();
    metrics.reset_requests_rejected();

    let api_router = Router::new()
        // V1 routes
        .route(
            "/v1/account/:owner/orders",
            axum::routing::get(get_user_orders::get_user_orders_handler)
        )
        .route(
            "/v1/app_data",
            axum::routing::put(put_app_data::put_app_data_without_hash)
                .layer(DefaultBodyLimit::max(app_data_size_limit))
        )
        .route(
            "/v1/app_data/:hash",
            axum::routing::get(get_app_data::get_app_data_handler)
                .merge(
                    axum::routing::put(put_app_data::put_app_data_with_hash)
                        .layer(DefaultBodyLimit::max(app_data_size_limit))
                ),
        )
        .route(
            "/v1/auction",
            axum::routing::get(get_auction::get_auction_handler)
        )
        .route(
            "/v1/orders",
            axum::routing::post(post_order::post_order_handler)
                .merge(
                    axum::routing::delete(cancel_orders::cancel_orders_handler)
                ),
        )
        .route(
            "/v1/orders/:uid",
            axum::routing::get(get_order_by_uid::get_order_by_uid_handler)
                .merge(
                    axum::routing::delete(cancel_order::cancel_order_handler)
                ),
        )
        .route(
            "/v1/orders/:uid/status",
            axum::routing::get(get_order_status::get_status_handler)
        )
        .route(
            "/v1/quote",
            axum::routing::post(post_quote::post_quote_handler)
        )
        // /solver_competition routes (specific before parameterized)
        .route(
            "/v1/solver_competition/latest",
            axum::routing::get(get_solver_competition::get_solver_competition_latest_handler)
        )
        .route(
            "/v1/solver_competition/by_tx_hash/:tx_hash",
            axum::routing::get(get_solver_competition::get_solver_competition_by_hash_handler)
        )
        .route(
            "/v1/solver_competition/:auction_id",
            axum::routing::get(get_solver_competition::get_solver_competition_by_id_handler)
        )
        .route(
            "/v1/token/:token/metadata",
            axum::routing::get(get_token_metadata::get_token_metadata_handler)
        )
        .route(
            "/v1/token/:token/native_price",
            axum::routing::get(get_native_price::get_native_price_handler)
        )
        .route(
            "/v1/trades",
            axum::routing::get(get_trades::get_trades_handler)
        )
        .route(
            "/v1/transactions/:hash/orders",
            axum::routing::get(get_orders_by_tx::get_orders_by_tx_handler)
        )
        .route(
            "/v1/users/:user/total_surplus",
            axum::routing::get(get_total_surplus::get_total_surplus_handler)
        )
        .route(
            "/v1/version",
            axum::routing::get(version::version_handler)
        )
        // V2 routes
        // /solver_competition routes (specific before parameterized)
        .route(
            "/v2/solver_competition/latest",
            axum::routing::get(get_solver_competition_v2::get_solver_competition_latest_handler)
        )
        .route(
            "/v2/solver_competition/by_tx_hash/:tx_hash",
            axum::routing::get(get_solver_competition_v2::get_solver_competition_by_hash_handler)
        )
        .route(
            "/v2/solver_competition/:auction_id",
            axum::routing::get(get_solver_competition_v2::get_solver_competition_by_id_handler)
        )
        .route(
            "/v2/trades",
            axum::routing::get(get_trades_v2::get_trades_handler)
        )
        .with_state(state)
        .layer(middleware::from_fn(with_matched_path_metric));

    finalize_router(api_router)
}

#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "api")]
struct ApiMetrics {
    /// Number of completed API requests.
    #[metric(labels("method", "status_code"))]
    requests_complete: prometheus::IntCounterVec,

    /// Number of rejected API requests.
    #[metric(labels("status_code"))]
    requests_rejected: prometheus::IntCounterVec,

    /// Execution time for each API request.
    #[metric(labels("method"), buckets(0.1, 0.5, 1, 2, 4, 6, 8, 10))]
    requests_duration_seconds: prometheus::HistogramVec,
}

impl ApiMetrics {
    // Status codes we care about in our application. Populated with:
    // `rg -oIN 'StatusCode::[A-Z_]+' | sort | uniq`.
    const INITIAL_STATUSES: &'static [StatusCode] = &[
        StatusCode::OK,
        StatusCode::CREATED,
        StatusCode::BAD_REQUEST,
        StatusCode::UNAUTHORIZED,
        StatusCode::FORBIDDEN,
        StatusCode::NOT_FOUND,
        StatusCode::INTERNAL_SERVER_ERROR,
        StatusCode::SERVICE_UNAVAILABLE,
    ];

    fn reset_requests_rejected(&self) {
        for status in Self::INITIAL_STATUSES {
            self.requests_rejected
                .with_label_values(&[status.as_str()])
                .reset();
        }
    }

    fn on_request_completed(&self, method: &str, status: StatusCode, timer: Instant) {
        self.requests_complete
            .with_label_values(&[method, status.as_str()])
            .inc();
        self.requests_duration_seconds
            .with_label_values(&[method])
            .observe(timer.elapsed().as_secs_f64());
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub error_type: Cow<'static, str>,
    pub description: Cow<'static, str>,
    /// Additional arbitrary data that can be attached to an API error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

pub fn error(error_type: &'static str, description: impl AsRef<str>) -> Json<Error> {
    Json(Error {
        error_type: error_type.into(),
        description: Cow::Owned(description.as_ref().to_owned()),
        data: None,
    })
}

pub fn rich_error(
    error_type: &'static str,
    description: impl AsRef<str>,
    data: impl Serialize,
) -> Json<Error> {
    let data = match serde_json::to_value(&data) {
        Ok(value) => Some(value),
        Err(err) => {
            tracing::warn!(?err, "failed to serialize error data");
            None
        }
    };

    Json(Error {
        error_type: error_type.into(),
        description: Cow::Owned(description.as_ref().to_owned()),
        data,
    })
}

pub fn internal_error_reply() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        error("InternalServerError", ""),
    )
        .into_response()
}

/// Sets up basic metrics, cors and proper log tracing for all routes.
/// Takes a router with versioned routes and nests under /api, then applies
/// middleware.
fn finalize_router(api_router: Router) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(vec![
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::DELETE,
            axum::http::Method::OPTIONS,
            axum::http::Method::PUT,
            axum::http::Method::PATCH,
            axum::http::Method::HEAD,
        ])
        .allow_headers(vec![
            axum::http::header::ORIGIN,
            axum::http::header::CONTENT_TYPE,
            // Must be lower case due to the HTTP-2 spec
            axum::http::HeaderName::from_static("x-auth-token"),
            axum::http::HeaderName::from_static("x-appid"),
        ]);

    let trace_layer = TraceLayer::new_for_http().make_span_with(tracing_axum::make_span);

    Router::new()
        .nest("/api", api_router)
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_PAYLOAD as usize))
        .layer(cors)
        .layer(trace_layer)
}

// Newtype wrapper for PriceEstimationError to allow IntoResponse implementation
// (orphan rules prevent implementing IntoResponse directly on external types)
pub(crate) struct PriceEstimationErrorWrapper(pub(crate) PriceEstimationError);

impl IntoResponse for PriceEstimationErrorWrapper {
    fn into_response(self) -> Response {
        match self.0 {
            PriceEstimationError::UnsupportedToken { token, reason } => (
                StatusCode::BAD_REQUEST,
                error(
                    "UnsupportedToken",
                    format!("Token {token:?} is unsupported: {reason:}"),
                ),
            )
                .into_response(),
            PriceEstimationError::UnsupportedOrderType(order_type) => (
                StatusCode::BAD_REQUEST,
                error(
                    "UnsupportedOrderType",
                    format!("{order_type} not supported"),
                ),
            )
                .into_response(),
            PriceEstimationError::NoLiquidity
            | PriceEstimationError::RateLimited
            | PriceEstimationError::EstimatorInternal(_) => (
                StatusCode::NOT_FOUND,
                error("NoLiquidity", "no route found"),
            )
                .into_response(),
            PriceEstimationError::ProtocolInternal(err) => {
                tracing::error!(?err, "PriceEstimationError::Other");
                internal_error_reply()
            }
        }
    }
}

// Implement From to allow easy conversion
impl From<PriceEstimationError> for PriceEstimationErrorWrapper {
    fn from(err: PriceEstimationError) -> Self {
        Self(err)
    }
}

#[cfg(test)]
pub async fn response_body<B>(response: axum::http::Response<B>) -> Vec<u8>
where
    B: axum::body::HttpBody + Unpin,
    B::Data: AsRef<[u8]>,
    B::Error: Debug,
{
    let mut body = response.into_body();
    let mut result = Vec::new();
    while let Some(frame) = body.data().await {
        let bytes = frame.unwrap();
        result.extend_from_slice(bytes.as_ref());
    }
    result
}

#[cfg(test)]
mod tests {
    use {super::*, serde::ser, serde_json::json};

    #[test]
    fn rich_errors_skip_unset_data_field() {
        assert_eq!(
            serde_json::to_value(&Error {
                error_type: "foo".into(),
                description: "bar".into(),
                data: None,
            })
            .unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
            }),
        );
        assert_eq!(
            serde_json::to_value(Error {
                error_type: "foo".into(),
                description: "bar".into(),
                data: Some(json!(42)),
            })
            .unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
                "data": 42,
            }),
        );
    }

    #[tokio::test]
    async fn rich_errors_handle_serialization_errors() {
        use axum::body::HttpBody;

        struct AlwaysErrors;
        impl Serialize for AlwaysErrors {
            fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(ser::Error::custom("error"))
            }
        }

        let response = rich_error("foo", "bar", AlwaysErrors).into_response();
        let mut body = response.into_body();
        let mut bytes = Vec::new();
        while let Some(frame) = body.data().await {
            let chunk = frame.unwrap();
            bytes.extend_from_slice(&chunk);
        }

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
            })
        );
    }
}
