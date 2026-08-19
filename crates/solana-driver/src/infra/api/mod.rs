//! HTTP API server.

use {
    crate::domain,
    axum::{
        Router,
        extract::DefaultBodyLimit,
        routing::{get, post},
    },
    cow_solana_rpc::SolanaRPC,
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::{net::SocketAddr, sync::Arc},
    tokio_util::sync::CancellationToken,
    tower::ServiceBuilder,
    tower_http::{decompression::RequestDecompressionLayer, trace::TraceLayer},
};

pub mod error;
pub mod routes;

pub use self::error::Error;

/// The Solana driver HTTP API server.
pub struct Api {
    /// Address the server binds to and listens on.
    pub addr: SocketAddr,
    /// The shared Solana RPC client.
    pub rpc: SolanaRPC,
    /// The competition that runs auctions across solver engines.
    pub competition: domain::Competition,
}

impl Api {
    /// Bind to the configured address, returning the listener and the actual
    /// bound address (which differs from `addr` when binding to port 0).
    pub async fn bind(&self) -> Result<(tokio::net::TcpListener, SocketAddr), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        tracing::info!(port = local_addr.port(), "serving solana driver");
        Ok((listener, local_addr))
    }

    /// Serve the API on the given listener until `shutdown` resolves, then
    /// drain in-flight requests before returning.
    pub async fn serve(
        self,
        listener: tokio::net::TcpListener,
        shutdown: CancellationToken,
    ) -> Result<(), std::io::Error> {
        // Propagate the OpenTelemetry trace context from incoming request headers and
        // record the trace id on the request span, so the driver can correlate logs
        // across services. `make_span` sets the parent context and an empty
        // `trace_id` field. `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        let state = State::new(self.rpc, self.competition);

        let app = Router::new()
            .route("/healthz", get(routes::healthz))
            .route("/solve", post(routes::solve))
            .route("/settle", post(routes::settle))
            // Disable the request body limit: solver payloads (auctions and solutions)
            // can exceed axum's 2MB default.
            .layer(DefaultBodyLimit::disable())
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
    /// Build the shared state the handlers operate on.
    fn new(rpc: SolanaRPC, competition: domain::Competition) -> Self {
        Self(Arc::new(Inner { rpc, competition }))
    }

    /// The competition that runs auctions across solver engines.
    fn competition(&self) -> &domain::Competition {
        &self.0.competition
    }
}

struct Inner {
    /// The shared Solana RPC client.
    #[expect(dead_code, reason = "used by the deadline and submission follow-ups")]
    rpc: SolanaRPC,
    /// The competition that runs auctions across solver engines.
    competition: domain::Competition,
}
