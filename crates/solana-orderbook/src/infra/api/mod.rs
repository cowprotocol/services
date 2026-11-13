//! HTTP API server.

use {
    super::{prices::NativePrices, quoter::Quoter, verifier::Verifier},
    axum::{
        Router,
        extract::MatchedPath,
        http::{self, StatusCode},
        middleware::{self, Next},
        response::Response,
        routing::{get, post, put},
    },
    chrono::{DateTime, Utc},
    configs::{fee_factor::FeeFactor, orderbook::VolumeFeeConfig},
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    sqlx::PgPool,
    std::{
        io,
        net::SocketAddr,
        num::NonZero,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::{net::TcpListener, sync::Mutex},
    tokio_util::sync::CancellationToken,
    tower::ServiceBuilder,
    tower_http::{
        cors::{Any, CorsLayer},
        decompression::RequestDecompressionLayer,
        trace::TraceLayer,
    },
};

pub mod error;
pub mod extract;
pub mod routes;

/// How long a block height read serves later requests. Polling clients then
/// share one RPC read, and a creation deadline is checked no coarser than
/// the height itself moves.
const BLOCK_HEIGHT_TTL: Duration = Duration::from_secs(2);

/// The Solana orderbook HTTP API server.
pub struct Api {
    /// Address the server binds to and listens on.
    pub addr: SocketAddr,
    /// The database the indexer writes to.
    pub pool: PgPool,
    /// The driver that quotes orders.
    pub quoter: Quoter,
    /// The autopilot's native prices, which gate quotes.
    pub prices: NativePrices,
    /// Bounds on a quoted order's `validTo`.
    pub validation: ValidationParameters,
    /// How long the quoted amounts are honored.
    pub quote_expiry: std::time::Duration,
    /// Drivers a `fast` quote waits for before it answers.
    pub fast_results_required: NonZero<usize>,
    /// The volume protocol fee quotes net out, absent when none is charged.
    pub volume_fee: Option<VolumeFeeConfig>,
    /// Sponsored placement dependencies, absent when the feature is off.
    pub sponsoring: Option<Sponsoring>,
    /// Settles quotes in simulation, absent when every quote is unverified.
    pub verifier: Option<Verifier>,
    /// Most bytes an app data document may have.
    pub app_data_size_limit: usize,
}

/// Bounds on an order's `validTo`, for quotes and sponsored placement
/// alike. The defaults are the EVM orderbook's.
#[derive(Clone, Copy, Debug)]
pub struct ValidationParameters {
    /// Least far in the future the `validTo` may lie.
    pub min_validity: std::time::Duration,
    /// Furthest in the future the `validTo` may lie.
    pub max_validity: std::time::Duration,
}

impl Default for ValidationParameters {
    fn default() -> Self {
        Self {
            min_validity: std::time::Duration::from_secs(60),
            max_validity: std::time::Duration::from_secs(365 * 24 * 60 * 60),
        }
    }
}

impl Api {
    /// Bind to the configured address, returning the listener and the actual
    /// bound address (which differs from `addr` when binding to port 0).
    pub async fn bind(&self) -> Result<(TcpListener, SocketAddr), io::Error> {
        let listener = TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        tracing::info!(port = local_addr.port(), "serving solana orderbook");
        Ok((listener, local_addr))
    }

    /// Serve the API on the given listener until `shutdown` resolves, then
    /// drain in-flight requests before returning.
    pub async fn serve(
        self,
        listener: TcpListener,
        shutdown: CancellationToken,
    ) -> Result<(), io::Error> {
        // Propagate the OpenTelemetry trace context from incoming request
        // headers and record the trace id on the request span, so logs
        // can be correlated across services. `make_span` sets the
        // parent context and an empty `trace_id` field;
        // `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        routes::initialize_metrics();
        let metrics = ApiMetrics::instance(observe::metrics::get_storage_registry()).unwrap();
        metrics.reset_requests_rejected();
        let state = State::new(
            self.pool,
            self.quoter,
            self.prices,
            self.validation,
            self.quote_expiry,
            self.fast_results_required,
            self.volume_fee,
            self.sponsoring,
            self.verifier,
            self.app_data_size_limit,
        );

        // Browsers call this API directly, so it answers cross-origin
        // requests like the EVM orderbook does.
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                http::Method::GET,
                http::Method::POST,
                http::Method::PUT,
                http::Method::OPTIONS,
                http::Method::HEAD,
            ])
            .allow_headers([http::header::ORIGIN, http::header::CONTENT_TYPE]);

        let endpoints = [
            ("GET", "/healthz", get(routes::healthz)),
            ("GET", "/api/v1/ready", get(routes::ready)),
            ("GET", "/api/v1/version", get(routes::version)),
            (
                "GET",
                "/api/v1/account/{owner}/orders",
                get(routes::account_orders),
            ),
            ("GET", "/api/v1/orders/{uid}", get(routes::order)),
            (
                "GET",
                "/api/v1/orders/{uid}/status",
                get(routes::order_status),
            ),
            ("GET", "/api/v2/trades", get(routes::trades)),
            (
                "GET",
                "/api/v2/solver_competition/latest",
                get(routes::solver_competition_latest),
            ),
            (
                "GET",
                "/api/v2/solver_competition/by_tx_hash/{tx_hash}",
                get(routes::solver_competition_by_tx_hash),
            ),
            (
                "GET",
                "/api/v2/solver_competition/{auction_id}",
                get(routes::solver_competition_by_id),
            ),
            (
                "GET",
                "/restricted/api/v2/solver_competition/{auction_id}",
                get(routes::solver_competition_by_id_unfiltered),
            ),
            ("POST", "/api/v1/orders", post(routes::create_order)),
            ("POST", "/api/v1/quote", post(routes::quote)),
            ("PUT", "/api/v1/app_data", put(routes::put_app_data)),
            ("GET", "/api/v1/app_data/{hash}", get(routes::get_app_data)),
            (
                "PUT",
                "/api/v1/app_data/{hash}",
                put(routes::put_app_data_by_hash),
            ),
        ];
        let mut app = Router::new();
        for (method, path, method_router) in endpoints {
            metrics.reset_requests_complete(method, path);
            app = app.route(path, method_router);
        }
        let app = app
            .layer(cors)
            .layer(RequestDecompressionLayer::new())
            .layer(middleware::from_fn(with_matched_path_metric))
            .layer(tracing_layer)
            .with_state(state);

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
    }
}

/// Shared state available to all route handlers.
#[derive(Clone)]
pub struct State(Arc<Inner>);

/// Sponsored order placement dependencies: the funder identity the incoming
/// transactions must commit to, the settlement program they must target, and
/// the RPC client that vouches for blockhash freshness and reads the order's
/// accounts.
pub struct Sponsoring {
    pub funder: solana_sdk::pubkey::Pubkey,
    pub settlement_program: solana_sdk::pubkey::Pubkey,
    pub rpc: cow_solana_rpc::SolanaRPC,
    /// The most the funder will pay in priority fee for one creation.
    pub max_priority_fee_lamports: u64,
    /// The verdicts on the mints placement and quoting read so far.
    pub mints: solana_token::MintVerdicts,
    /// How long placement waits for the node to confirm a blockhash it does
    /// not know.
    pub blockhash_catch_up: Duration,
}

impl State {
    #[expect(clippy::too_many_arguments)]
    fn new(
        pool: PgPool,
        quoter: Quoter,
        prices: NativePrices,
        validation: ValidationParameters,
        quote_expiry: std::time::Duration,
        fast_results_required: NonZero<usize>,
        volume_fee: Option<VolumeFeeConfig>,
        sponsoring: Option<Sponsoring>,
        verifier: Option<Verifier>,
        app_data_size_limit: usize,
    ) -> Self {
        Self(Arc::new(Inner {
            pool,
            quoter,
            prices,
            validation,
            quote_expiry,
            fast_results_required,
            volume_fee,
            sponsoring,
            verifier,
            app_data_size_limit,
            block_height: Mutex::default(),
        }))
    }

    /// The database handle the order, trades, and auction endpoints read
    /// from.
    pub fn pool(&self) -> &PgPool {
        &self.0.pool
    }

    /// The driver that quotes orders.
    pub fn quoter(&self) -> &Quoter {
        &self.0.quoter
    }

    /// The autopilot's native prices, which gate quotes.
    pub fn prices(&self) -> &NativePrices {
        &self.0.prices
    }

    /// Bounds on a quoted order's `validTo`.
    pub fn validation(&self) -> ValidationParameters {
        self.0.validation
    }

    /// How long the quoted amounts are honored.
    pub fn quote_expiry(&self) -> std::time::Duration {
        self.0.quote_expiry
    }

    /// Drivers a `fast` quote waits for before it answers.
    pub fn fast_results_required(&self) -> NonZero<usize> {
        self.0.fast_results_required
    }

    /// The volume protocol fee factor in force at `now`, `None` when no
    /// protocol fee is charged.
    pub fn volume_fee(&self, now: DateTime<Utc>) -> Option<FeeFactor> {
        self.0
            .volume_fee
            .as_ref()
            .filter(|fee| fee.effective_from_timestamp.is_none_or(|from| from <= now))
            .and_then(|fee| fee.factor)
    }

    /// Sponsored placement dependencies, absent when the feature is off.
    pub fn sponsoring(&self) -> Option<&Sponsoring> {
        self.0.sponsoring.as_ref()
    }

    /// Settles quotes in simulation, absent when every quote is unverified.
    pub fn verifier(&self) -> Option<&Verifier> {
        self.0.verifier.as_ref()
    }

    /// Most bytes an app data document may have.
    pub fn app_data_size_limit(&self) -> usize {
        self.0.app_data_size_limit
    }

    /// The chain's block height, which decides whether a pending sponsored
    /// creation can still land, read at most once per `BLOCK_HEIGHT_TTL`.
    /// `None` without sponsoring, which stores no creation deadlines, or
    /// when the read fails: the deadline check is skipped rather than the
    /// request failed.
    pub async fn block_height(&self) -> Option<i64> {
        let sponsoring = self.sponsoring()?;
        // Held across the read so concurrent requests share it.
        let mut cached = self.0.block_height.lock().await;
        if let Some((read_at, height)) = *cached
            && read_at.elapsed() < BLOCK_HEIGHT_TTL
        {
            return Some(height);
        }
        match sponsoring.rpc.block_height().await {
            Ok(height) => {
                let height = i64::try_from(u64::from(height)).ok()?;
                *cached = Some((Instant::now(), height));
                Some(height)
            }
            Err(err) => {
                tracing::warn!(
                    ?err,
                    "block height read failed, creation deadlines unchecked"
                );
                None
            }
        }
    }
}

struct Inner {
    /// The database the indexer writes to.
    pool: PgPool,
    /// The driver that quotes orders.
    quoter: Quoter,
    /// The autopilot's native prices, which gate quotes.
    prices: NativePrices,
    /// Bounds on a quoted order's `validTo`.
    validation: ValidationParameters,
    /// How long the quoted amounts are honored.
    quote_expiry: std::time::Duration,
    /// Drivers a `fast` quote waits for before it answers.
    fast_results_required: NonZero<usize>,
    /// The volume protocol fee quotes net out, absent when none is charged.
    volume_fee: Option<VolumeFeeConfig>,
    /// Sponsored placement dependencies, absent when the feature is off.
    sponsoring: Option<Sponsoring>,
    /// Settles quotes in simulation, absent when every quote is unverified.
    verifier: Option<Verifier>,
    /// Most bytes an app data document may have.
    app_data_size_limit: usize,
    /// The last block height read and when, see [`State::block_height`].
    block_height: Mutex<Option<(Instant, i64)>>,
}

/// Counts and times every request by method and matched route.
async fn with_matched_path_metric(req: http::Request<axum::body::Body>, next: Next) -> Response {
    let metrics = ApiMetrics::instance(observe::metrics::get_storage_registry()).unwrap();

    let http_method = req.method().as_str();
    let matched_path = req
        .extensions()
        .get::<MatchedPath>()
        .map(|path| path.as_str())
        .unwrap_or("unknown");
    let method_with_path = format!("{http_method} {matched_path}");

    metrics
        .requests_started
        .with_label_values(&[method_with_path.as_str()])
        .inc();

    let response = {
        let _timer = metrics
            .requests_duration_seconds
            .with_label_values(&[&method_with_path])
            .start_timer();
        next.run(req).await
    };
    let status = response.status();

    metrics
        .requests_complete
        .with_label_values(&[method_with_path.as_str(), status.as_str()])
        .inc();

    if status.is_client_error() || status.is_server_error() {
        metrics
            .requests_rejected
            .with_label_values(&[status.as_str()])
            .inc();
    }

    response
}

/// API request metrics. The `method` label holds the HTTP method and the
/// matched route, e.g. `POST /api/v1/quote`.
#[derive(prometheus_metric_storage::MetricStorage, Clone, Debug)]
#[metric(subsystem = "api")]
struct ApiMetrics {
    /// Number of started API requests.
    #[metric(labels("method"))]
    requests_started: prometheus::IntCounterVec,

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
    /// Status codes the routes answer with.
    const INITIAL_STATUSES: &'static [StatusCode] = &[
        StatusCode::OK,
        StatusCode::CREATED,
        StatusCode::BAD_REQUEST,
        StatusCode::NOT_FOUND,
        StatusCode::TOO_MANY_REQUESTS,
        StatusCode::INTERNAL_SERVER_ERROR,
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
