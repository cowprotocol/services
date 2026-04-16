use {
    crate::{
        app_data,
        database::Postgres,
        orderbook::Orderbook,
        quoter::QuoteHandler,
        solver_competition::LoadSolverCompetitionError,
    },
    axum::{
        Router,
        extract::DefaultBodyLimit,
        http::{Request, StatusCode, header::USER_AGENT},
        middleware::{self, Next},
        response::{IntoResponse, Json, Response},
        routing::{delete, get, post, put},
    },
    ethrpc::block_stream::CurrentBlockWatcher,
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    price_estimation::{PriceEstimationError, native::NativePriceEstimating},
    serde::{Deserialize, Serialize},
    std::{
        borrow::Cow,
        fmt::Debug,
        sync::Arc,
        time::{Duration, Instant},
    },
    tower::ServiceBuilder,
    tower_http::{cors::CorsLayer, trace::TraceLayer},
};

mod cancel_order;
mod cancel_orders;
mod debug_order;
mod debug_simulation;
mod get_app_data;
mod get_auction;
mod get_native_price;
mod get_order_by_uid;
mod get_order_status;
mod get_orders_by_tx;
mod get_orders_by_uid;
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

const ALLOWED_METHODS: &[axum::http::Method] = &[
    axum::http::Method::GET,
    axum::http::Method::POST,
    axum::http::Method::DELETE,
    axum::http::Method::OPTIONS,
    axum::http::Method::PUT,
    axum::http::Method::PATCH,
    axum::http::Method::HEAD,
];

/// Centralized application state shared across all API handlers
pub struct AppState {
    pub database_write: Postgres,
    pub database_read: Postgres,
    pub orderbook: Arc<Orderbook>,
    pub quotes: QuoteHandler,
    pub app_data: Arc<app_data::Registry>,
    pub native_price_estimator: Arc<dyn NativePriceEstimating>,
    pub quote_timeout: Duration,
    pub current_block_stream: CurrentBlockWatcher,
    pub hide_competition_before_deadline: bool,
}

impl AppState {
    /// When the feature is enabled, returns the current block number so DB
    /// queries can hide competition data whose deadline hasn't passed yet.
    /// Returns `None` when the feature is off (no filtering).
    pub(crate) fn hide_competition_before_block(&self) -> Option<i64> {
        self.hide_competition_before_deadline
            .then(|| self.current_block_stream.borrow().number.cast_signed())
    }
}

async fn summarize_request(req: Request<axum::body::Body>, next: Next) -> Response {
    let method = req.method().to_string();
    let uri = req.uri().to_string();

    let user_agent = req
        .headers()
        .get(USER_AGENT)
        .map(|user_agent| user_agent.to_str().unwrap_or("invalid (non-ASCII)"))
        .unwrap_or("unset")
        .to_string();

    let timer = Instant::now();
    let response = next.run(req).await;
    let status = response.status().as_u16();

    tracing::info!(
        method,
        uri,
        user_agent,
        status,
        elapsed = ?timer.elapsed(),
        "request_summary",
    );

    response
}

/// Middleware that automatically tracks metrics using Axum's MatchedPath
async fn with_matched_path_metric(req: Request<axum::body::Body>, next: Next) -> Response {
    let metrics = ApiMetrics::instance(observe::metrics::get_storage_registry()).unwrap();

    let http_method = req.method().as_str();
    let matched_path = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str())
        .unwrap_or("unknown");
    let method_with_path = format!("{http_method} {matched_path}");

    let response = {
        let _timer = metrics
            .requests_duration_seconds
            .with_label_values(&[&method_with_path])
            .start_timer();
        next.run(req).await
    };
    let status = response.status();

    // Track completed requests
    metrics
        .requests_complete
        .with_label_values(&[method_with_path.as_str(), status.as_str()])
        .inc();

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

#[expect(clippy::too_many_arguments)]
pub fn handle_all_routes(
    database_write: Postgres,
    database_read: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: QuoteHandler,
    app_data: Arc<app_data::Registry>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    quote_timeout: Duration,
    current_block_stream: CurrentBlockWatcher,
    hide_competition_before_deadline: bool,
) -> Router {
    let app_data_size_limit = app_data.size_limit();

    let state = Arc::new(AppState {
        database_write,
        database_read,
        orderbook,
        quotes,
        app_data,
        native_price_estimator,
        quote_timeout,
        current_block_stream,
        hide_competition_before_deadline,
    });

    let routes = [
        // V1 routes
        (
            "GET",
            "/api/v1/account/{owner}/orders",
            get(get_user_orders::get_user_orders_handler),
        ),
        (
            "PUT",
            "/api/v1/app_data",
            put(put_app_data::put_app_data_without_hash)
                .layer(DefaultBodyLimit::max(app_data_size_limit)),
        ),
        (
            "GET",
            "/api/v1/app_data/{hash}",
            get(get_app_data::get_app_data_handler),
        ),
        (
            "PUT",
            "/api/v1/app_data/{hash}",
            put(put_app_data::put_app_data_with_hash)
                .layer(DefaultBodyLimit::max(app_data_size_limit)),
        ),
        (
            "GET",
            "/api/v1/auction",
            get(get_auction::get_auction_handler),
        ),
        (
            "POST",
            "/api/v1/orders",
            post(post_order::post_order_handler),
        ),
        (
            "POST",
            "/api/v1/orders/by_uids",
            post(get_orders_by_uid::get_orders_by_uid_handler),
        ),
        (
            "DELETE",
            "/api/v1/orders",
            delete(cancel_orders::cancel_orders_handler),
        ),
        (
            "GET",
            "/api/v1/orders/{uid}",
            get(get_order_by_uid::get_order_by_uid_handler),
        ),
        (
            "DELETE",
            "/api/v1/orders/{uid}",
            delete(cancel_order::cancel_order_handler),
        ),
        (
            "GET",
            "/api/v1/orders/{uid}/status",
            get(get_order_status::get_status_handler),
        ),
        (
            "POST",
            "/api/v1/quote",
            post(post_quote::post_quote_handler),
        ),
        // /solver_competition routes (specific before parameterized)
        (
            "GET",
            "/api/v1/solver_competition/latest",
            get(get_solver_competition::get_solver_competition_latest_handler),
        ),
        (
            "GET",
            "/api/v1/solver_competition/by_tx_hash/{tx_hash}",
            get(get_solver_competition::get_solver_competition_by_hash_handler),
        ),
        (
            "GET",
            "/api/v1/solver_competition/{auction_id}",
            get(get_solver_competition::get_solver_competition_by_id_handler),
        ),
        (
            "GET",
            "/api/v1/token/{token}/metadata",
            get(get_token_metadata::get_token_metadata_handler),
        ),
        (
            "GET",
            "/api/v1/token/{token}/native_price",
            get(get_native_price::get_native_price_handler),
        ),
        ("GET", "/api/v1/trades", get(get_trades::get_trades_handler)),
        (
            "GET",
            "/api/v1/transactions/{hash}/orders",
            get(get_orders_by_tx::get_orders_by_tx_handler),
        ),
        (
            "GET",
            "/api/v1/users/{user}/total_surplus",
            get(get_total_surplus::get_total_surplus_handler),
        ),
        ("GET", "/api/v1/version", get(version::version_handler)),
        (
            "GET",
            "/api/internal/v1/debug/order/{uid}",
            get(debug_order::debug_order_handler),
        ),
        (
            "GET",
            "/api/internal/v1/debug/simulation/{uid}",
            get(debug_simulation::debug_simulation_handler),
        ),
        (
            "POST",
            "/api/internal/v1/debug/simulation",
            post(debug_simulation::debug_simulation_post_handler),
        ),
        // V2 routes
        // /solver_competition routes (specific before parameterized)
        (
            "GET",
            "/api/v2/solver_competition/latest",
            get(get_solver_competition_v2::get_solver_competition_latest_handler),
        ),
        (
            "GET",
            "/api/v2/solver_competition/by_tx_hash/{tx_hash}",
            get(get_solver_competition_v2::get_solver_competition_by_hash_handler),
        ),
        (
            "GET",
            "/api/v2/solver_competition/{auction_id}",
            get(get_solver_competition_v2::get_solver_competition_by_id_handler),
        ),
        (
            "GET",
            "/api/v2/trades",
            get(get_trades_v2::get_trades_handler),
        ),
    ];

    // Initialize metrics
    let metrics = ApiMetrics::instance(observe::metrics::get_storage_registry()).unwrap();
    metrics.reset_requests_rejected();

    let mut api_router = Router::new();
    for (method, path, method_router) in routes {
        metrics.reset_requests_complete(method, path);
        api_router = api_router.route(path, method_router);
    }
    let api_router = api_router.with_state(state);

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(ALLOWED_METHODS.to_vec())
        .allow_headers(vec![
            axum::http::header::ORIGIN,
            axum::http::header::CONTENT_TYPE,
            // Must be lower case due to the HTTP-2 spec
            axum::http::HeaderName::from_static("x-auth-token"),
            axum::http::HeaderName::from_static("x-appid"),
        ]);

    api_router
        .layer(DefaultBodyLimit::max(MAX_JSON_BODY_PAYLOAD as usize))
        .layer(cors)
        .layer(middleware::from_fn(summarize_request))
        .layer(middleware::from_fn(with_matched_path_metric))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http().make_span_with(make_span))
                .map_request(record_trace_id),
        )
}

// NOTE(jmg-duarte): method is actually the request path, to avoid breaking
// dashboards, the http_method was added
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

    fn reset_requests_complete(&self, method: &str, path: &str) {
        let method_with_path = format!("{method} {path}");
        for status in Self::INITIAL_STATUSES {
            self.requests_complete
                .with_label_values(&[method_with_path.as_str(), status.as_str()])
                .reset();
        }
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
            PriceEstimationError::TradingOutsideAllowedWindow { message } => (
                StatusCode::BAD_REQUEST,
                error("TradingOutsideAllowedWindow", message),
            )
                .into_response(),
            PriceEstimationError::TokenTemporarilySuspended { message } => (
                StatusCode::BAD_REQUEST,
                error("TokenTemporarilySuspended", message),
            )
                .into_response(),
            PriceEstimationError::InsufficientLiquidity { message } => (
                StatusCode::BAD_REQUEST,
                error("InsufficientLiquidity", message),
            )
                .into_response(),
            PriceEstimationError::CustomSolverError { message } => {
                (StatusCode::BAD_REQUEST, error("CustomSolverError", message)).into_response()
            }
            PriceEstimationError::ProtocolInternal(err) => {
                tracing::error!(?err, "PriceEstimationError::Other");
                internal_error_reply()
            }
        }
    }
}

impl IntoResponse for LoadSolverCompetitionError {
    fn into_response(self) -> Response {
        match self {
            err @ LoadSolverCompetitionError::NotFound => {
                (StatusCode::NOT_FOUND, error("NotFound", err.to_string())).into_response()
            }
            LoadSolverCompetitionError::Other(err) => {
                tracing::error!(?err, "failed to load solver competition");
                internal_error_reply()
            }
        }
    }
}

#[cfg(test)]
pub async fn response_body(response: axum::http::Response<axum::body::Body>) -> Vec<u8> {
    // SAFETY: usize::MAX is ok here because it's a test
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

#[cfg(test)]
mod tests {
    use {
        crate::api::{Error, PriceEstimationErrorWrapper, rich_error},
        alloy::primitives::{Address, B256},
        app_data::AppDataHash,
        axum::{
            Router,
            body::Body,
            extract::{Path, Query},
            http::{Request, StatusCode},
            response::IntoResponse,
            routing::get,
        },
        model::order::OrderUid,
        price_estimation::PriceEstimationError,
        serde::{Deserialize, Serialize, ser},
        serde_json::json,
        tower::ServiceExt as _,
    };

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
        // SAFETY: usize::MAX is ok here because it's a test
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();

        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            json!({
                "errorType": "foo",
                "description": "bar",
            })
        );
    }

    #[tokio::test]
    async fn maps_custom_price_estimation_errors_to_bad_request_responses() {
        let cases = [
            (
                PriceEstimationError::TradingOutsideAllowedWindow {
                    message: "window closed".to_string(),
                },
                "TradingOutsideAllowedWindow",
                "window closed",
            ),
            (
                PriceEstimationError::TokenTemporarilySuspended {
                    message: "token suspended".to_string(),
                },
                "TokenTemporarilySuspended",
                "token suspended",
            ),
            (
                PriceEstimationError::InsufficientLiquidity {
                    message: "insufficient liquidity".to_string(),
                },
                "InsufficientLiquidity",
                "insufficient liquidity",
            ),
            (
                PriceEstimationError::CustomSolverError {
                    message: "custom solver reason".to_string(),
                },
                "CustomSolverError",
                "custom solver reason",
            ),
        ];

        for (err, expected_type, expected_description) in cases {
            let response = PriceEstimationErrorWrapper(err).into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);

            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            let body: Error = serde_json::from_slice(&bytes).unwrap();

            assert_eq!(body.error_type, expected_type);
            assert_eq!(body.description.as_ref(), expected_description);
        }
    }

    // Tests for Axum extractor type parsing.
    //
    // Since the parsing behavior depends on the type, not the endpoint,
    // we test each type once here rather than duplicating across handlers.

    async fn get_request(router: Router, path: &str) -> axum::response::Response<Body> {
        router
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    mod path_order_uid {
        use super::*;

        async fn handler(Path(_uid): Path<OrderUid>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/orders/{uid}", get(handler))
        }

        async fn request(uid: &str) -> axum::response::Response<Body> {
            get_request(router(), &format!("/orders/{uid}")).await
        }

        #[tokio::test]
        async fn with_0x_prefix() {
            let uid = format!("0x{}", "01".repeat(56));
            assert_eq!(request(&uid).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn without_0x_prefix() {
            let uid = "01".repeat(56);
            assert_eq!(request(&uid).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn odd_hex_chars_with_0x() {
            let uid = format!("0x{}", "01".repeat(56).strip_suffix('1').unwrap());
            assert_eq!(request(&uid).await.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn odd_hex_chars_without_0x() {
            let uid = "01".repeat(56);
            let uid = uid.strip_suffix('1').unwrap();
            assert_eq!(request(uid).await.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod path_address {
        use super::*;

        async fn handler(Path(_addr): Path<Address>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/token/{addr}", get(handler))
        }

        async fn request(addr: &str) -> axum::response::Response<Body> {
            get_request(router(), &format!("/token/{addr}")).await
        }

        #[tokio::test]
        async fn with_0x_prefix() {
            let addr = format!("0x{}", "01".repeat(20));
            assert_eq!(request(&addr).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn without_0x_prefix() {
            let addr = "01".repeat(20);
            assert_eq!(request(&addr).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn odd_hex_chars_with_0x() {
            let addr = format!("0x{}", "01".repeat(20).strip_suffix('1').unwrap());
            assert_eq!(request(&addr).await.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn odd_hex_chars_without_0x() {
            let addr = "01".repeat(20);
            let addr = addr.strip_suffix('1').unwrap();
            assert_eq!(request(addr).await.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn too_short() {
            assert_eq!(request("0x0101").await.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn invalid_hex_chars() {
            let addr = format!("0x{}", "GG".repeat(20));
            assert_eq!(request(&addr).await.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod path_b256 {
        use super::*;

        async fn handler(Path(_hash): Path<B256>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/tx/{hash}", get(handler))
        }

        async fn request(hash: &str) -> axum::response::Response<Body> {
            get_request(router(), &format!("/tx/{hash}")).await
        }

        #[tokio::test]
        async fn with_0x_prefix() {
            let hash = format!("0x{}", "01".repeat(32));
            assert_eq!(request(&hash).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn without_0x_prefix() {
            let hash = "01".repeat(32);
            assert_eq!(request(&hash).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn odd_hex_chars_with_0x() {
            let hash = format!("0x{}", "01".repeat(32).strip_suffix('1').unwrap());
            assert_eq!(request(&hash).await.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn odd_hex_chars_without_0x() {
            let hash = "01".repeat(32);
            let hash = hash.strip_suffix('1').unwrap();
            assert_eq!(request(hash).await.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod path_app_data_hash {
        use super::*;

        async fn handler(Path(_hash): Path<AppDataHash>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/app_data/{hash}", get(handler))
        }

        async fn request(hash: &str) -> axum::response::Response<Body> {
            get_request(router(), &format!("/app_data/{hash}")).await
        }

        #[tokio::test]
        async fn with_0x_prefix() {
            let hash = format!("0x{}", "01".repeat(32));
            assert_eq!(request(&hash).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn without_0x_prefix() {
            let hash = "01".repeat(32);
            assert_eq!(request(&hash).await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn odd_hex_chars_with_0x() {
            let hash = format!("0x{}", "01".repeat(32).strip_suffix('1').unwrap());
            assert_eq!(request(&hash).await.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn odd_hex_chars_without_0x() {
            let hash = "01".repeat(32);
            let hash = hash.strip_suffix('1').unwrap();
            assert_eq!(request(hash).await.status(), StatusCode::BAD_REQUEST);
        }
    }

    mod path_u64 {
        use super::*;

        async fn handler(Path(_id): Path<u64>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/resource/{id}", get(handler))
        }

        async fn request(path: &str) -> axum::response::Response<Body> {
            get_request(router(), path).await
        }

        #[tokio::test]
        async fn valid() {
            assert_eq!(request("/resource/123").await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn invalid_string() {
            assert_eq!(
                request("/resource/abc").await.status(),
                StatusCode::BAD_REQUEST
            );
        }

        #[tokio::test]
        async fn negative() {
            assert_eq!(
                request("/resource/-1").await.status(),
                StatusCode::BAD_REQUEST
            );
        }
    }

    mod query_hex_types {
        use super::*;

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            #[allow(unused)]
            order_uid: Option<OrderUid>,
            #[allow(unused)]
            owner: Option<Address>,
        }

        async fn handler(Query(_q): Query<Params>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/trades", get(handler))
        }

        async fn request(query: &str) -> axum::response::Response<Body> {
            get_request(router(), &format!("/trades?{query}")).await
        }

        #[tokio::test]
        async fn order_uid_with_0x_prefix() {
            let uid = format!("0x{}", "01".repeat(56));
            assert_eq!(
                request(&format!("orderUid={uid}")).await.status(),
                StatusCode::OK
            );
        }

        #[tokio::test]
        async fn order_uid_without_0x_prefix() {
            let uid = "01".repeat(56);
            assert_eq!(
                request(&format!("orderUid={uid}")).await.status(),
                StatusCode::OK
            );
        }

        #[tokio::test]
        async fn order_uid_odd_hex_chars() {
            let uid = format!("0x{}", "01".repeat(56).strip_suffix('1').unwrap());
            assert_eq!(
                request(&format!("orderUid={uid}")).await.status(),
                StatusCode::BAD_REQUEST
            );
        }

        #[tokio::test]
        async fn owner_with_0x_prefix() {
            let owner = format!("0x{}", "01".repeat(20));
            assert_eq!(
                request(&format!("owner={owner}")).await.status(),
                StatusCode::OK
            );
        }

        #[tokio::test]
        async fn owner_without_0x_prefix() {
            let owner = "01".repeat(20);
            assert_eq!(
                request(&format!("owner={owner}")).await.status(),
                StatusCode::OK
            );
        }

        #[tokio::test]
        async fn owner_odd_hex_chars() {
            let owner = format!("0x{}", "01".repeat(20).strip_suffix('1').unwrap());
            assert_eq!(
                request(&format!("owner={owner}")).await.status(),
                StatusCode::BAD_REQUEST
            );
        }
    }

    mod query_numeric_types {
        use super::*;

        #[derive(Deserialize)]
        struct Params {
            #[allow(unused)]
            offset: Option<u64>,
            #[allow(unused)]
            limit: Option<u64>,
        }

        async fn handler(Query(_q): Query<Params>) -> StatusCode {
            StatusCode::OK
        }

        fn router() -> Router {
            Router::new().route("/items", get(handler))
        }

        async fn request(path: &str) -> axum::response::Response<Body> {
            get_request(router(), path).await
        }

        #[tokio::test]
        async fn no_params() {
            assert_eq!(request("/items").await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn only_offset() {
            assert_eq!(request("/items?offset=5").await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn only_limit() {
            assert_eq!(request("/items?limit=20").await.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn both_params() {
            assert_eq!(
                request("/items?offset=5&limit=20").await.status(),
                StatusCode::OK
            );
        }

        #[tokio::test]
        async fn invalid_offset() {
            assert_eq!(
                request("/items?offset=abc").await.status(),
                StatusCode::BAD_REQUEST
            );
        }

        #[tokio::test]
        async fn invalid_limit() {
            assert_eq!(
                request("/items?limit=abc").await.status(),
                StatusCode::BAD_REQUEST
            );
        }
    }
}
