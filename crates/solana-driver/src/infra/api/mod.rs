//! HTTP API server.

use {
    axum::{
        Router,
        extract::DefaultBodyLimit,
        routing::{get, post},
    },
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    solana_client::nonblocking::rpc_client::RpcClient,
    std::{net::SocketAddr, sync::Arc},
    tokio_util::sync::CancellationToken,
    tower::ServiceBuilder,
    tower_http::{decompression::RequestDecompressionLayer, trace::TraceLayer},
};

pub mod routes;

/// The Solana driver HTTP API server.
pub struct Api {
    /// Address the server binds to and listens on.
    pub addr: SocketAddr,
}

impl Api {
    /// Bind to the configured address, returning the listener and the actual
    /// bound address (which differs from `addr` when binding to port 0).
    pub async fn bind(self) -> Result<(tokio::net::TcpListener, SocketAddr), std::io::Error> {
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        tracing::info!(port = local_addr.port(), "serving solana driver");
        Ok((listener, local_addr))
    }

    /// Serve the API on the given listener until `shutdown` resolves, then
    /// drain in-flight requests before returning.
    pub async fn serve(
        listener: tokio::net::TcpListener,
        rpc: Arc<RpcClient>,
        shutdown: CancellationToken,
    ) -> Result<(), std::io::Error> {
        // Propagate the OpenTelemetry trace context from incoming request headers and
        // record the trace id on the request span, so logs can be correlated across
        // services. `make_span` sets the parent context and an empty `trace_id` field;
        // `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        let app = Router::new()
            .route("/healthz", get(routes::healthz))
            .route("/solve", post(routes::solve))
            .route("/settle", post(routes::settle))
            // Disable the request body limit: solver payloads (auctions and solutions)
            // can exceed axum's 2MB default.
            .layer(DefaultBodyLimit::disable())
            .layer(RequestDecompressionLayer::new())
            .layer(tracing_layer)
            .with_state(State(Arc::new(Inner { rpc })));

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
    }
}

/// Shared state available to all route handlers.
///
/// The inner field is not yet read by any handler (the `/solve` and `/settle`
/// handlers are stubs), so `#[expect(dead_code)]` suppresses the unused-field
/// warning until shared state is added.
#[derive(Clone)]
#[expect(dead_code)]
pub struct State(Arc<Inner>);

struct Inner {
    /// The shared Solana RPC client.
    #[expect(dead_code)]
    rpc: Arc<RpcClient>,
}
