//! HTTP API server.

use {
    axum::{Router, http, routing::get},
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
        // Propagate the OpenTelemetry trace context from incoming request headers and
        // record the trace id on the request span, so logs can be correlated across
        // services. `make_span` sets the parent context and an empty `trace_id` field;
        // `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        let state = State::new(self.pool);

        // Browsers call this API directly, so it answers cross-origin
        // requests like the EVM orderbook does.
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods([http::Method::GET, http::Method::OPTIONS, http::Method::HEAD])
            .allow_headers([http::header::ORIGIN, http::header::CONTENT_TYPE]);

        let app = Router::new()
            .route("/healthz", get(routes::healthz))
            .route("/api/v1/orders/{uid}", get(routes::order))
            .route("/api/v1/orders/{uid}/status", get(routes::order_status))
            .route("/api/v1/trades", get(routes::trades))
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

impl State {
    fn new(pool: PgPool) -> Self {
        Self(Arc::new(Inner { pool }))
    }

    /// The database handle the order, trades, and auction endpoints read
    /// from.
    pub fn pool(&self) -> &PgPool {
        &self.0.pool
    }
}

struct Inner {
    /// The database the indexer writes to.
    pool: PgPool,
}
