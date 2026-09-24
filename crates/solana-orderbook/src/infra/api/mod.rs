//! HTTP API server.

use {
    super::quoter::Quoter,
    axum::{
        Router,
        http,
        routing::{get, post},
    },
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    sqlx::PgPool,
    std::{io, net::SocketAddr, sync::Arc},
    tokio::net::TcpListener,
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

/// The Solana orderbook HTTP API server.
pub struct Api {
    /// Address the server binds to and listens on.
    pub addr: SocketAddr,
    /// The database the indexer writes to.
    pub pool: PgPool,
    /// The driver that quotes orders.
    pub quoter: Quoter,
    /// Bounds on a quoted order's `validTo`.
    pub validation: ValidationParameters,
    /// How long the quoted amounts are honored.
    pub quote_expiry: std::time::Duration,
    /// Sponsored placement dependencies, absent when the feature is off.
    pub sponsoring: Option<Sponsoring>,
}

/// Bounds on a quoted order's `validTo`. The defaults are the EVM
/// orderbook's.
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
            min_validity: std::time::Duration::from_secs(2 * 60),
            max_validity: std::time::Duration::from_secs(2 * 60 * 60),
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

        let state = State::new(
            self.pool,
            self.quoter,
            self.validation,
            self.quote_expiry,
            self.sponsoring,
        );

        // Browsers call this API directly, so it answers cross-origin
        // requests like the EVM orderbook does.
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([
                http::Method::GET,
                http::Method::POST,
                http::Method::OPTIONS,
                http::Method::HEAD,
            ])
            .allow_headers([http::header::ORIGIN, http::header::CONTENT_TYPE]);

        let app = Router::new()
            .route("/healthz", get(routes::healthz))
            .route(
                "/api/v1/account/{owner}/orders",
                get(routes::account_orders),
            )
            .route(
                "/api/v1/orders/{uid}",
                get(routes::order).delete(routes::cancel_order),
            )
            .route("/api/v1/orders/{uid}/status", get(routes::order_status))
            .route("/api/v2/trades", get(routes::trades))
            .route("/api/v1/orders", post(routes::create_order))
            .route("/api/v1/quote", post(routes::quote))
            .layer(cors)
            .layer(RequestDecompressionLayer::new())
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
/// the RPC client that vouches for blockhash freshness.
pub struct Sponsoring {
    pub funder: solana_sdk::pubkey::Pubkey,
    pub settlement_program: solana_sdk::pubkey::Pubkey,
    pub rpc: cow_solana_rpc::SolanaRPC,
    /// The most the funder will pay in priority fee for one creation.
    pub max_priority_fee_lamports: u64,
}

impl State {
    fn new(
        pool: PgPool,
        quoter: Quoter,
        validation: ValidationParameters,
        quote_expiry: std::time::Duration,
        sponsoring: Option<Sponsoring>,
    ) -> Self {
        Self(Arc::new(Inner {
            pool,
            quoter,
            validation,
            quote_expiry,
            sponsoring,
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

    /// Bounds on a quoted order's `validTo`.
    pub fn validation(&self) -> ValidationParameters {
        self.0.validation
    }

    /// How long the quoted amounts are honored.
    pub fn quote_expiry(&self) -> std::time::Duration {
        self.0.quote_expiry
    }

    /// Sponsored placement dependencies, absent when the feature is off.
    pub fn sponsoring(&self) -> Option<&Sponsoring> {
        self.0.sponsoring.as_ref()
    }
}

struct Inner {
    /// The database the indexer writes to.
    pool: PgPool,
    /// The driver that quotes orders.
    quoter: Quoter,
    /// Bounds on a quoted order's `validTo`.
    validation: ValidationParameters,
    /// How long the quoted amounts are honored.
    quote_expiry: std::time::Duration,
    /// Sponsored placement dependencies, absent when the feature is off.
    sponsoring: Option<Sponsoring>,
}
