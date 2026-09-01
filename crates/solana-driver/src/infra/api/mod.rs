//! HTTP API server.

use {
    crate::{
        domain,
        infra::{blockchain::Solana, solver::Solver},
    },
    axum::{Router, extract::DefaultBodyLimit, routing::get},
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::{net::SocketAddr, sync::Arc},
    tokio_util::sync::CancellationToken,
    tower::ServiceBuilder,
    tower_http::{decompression::RequestDecompressionLayer, trace::TraceLayer},
};

pub mod error;
pub mod extract;
pub mod routes;

pub use self::{error::Error, extract::LoggingJson};

/// The Solana driver HTTP API server.
pub struct Api {
    /// Address the server binds to and listens on.
    pub addr: SocketAddr,
    /// The shared Solana blockchain adapter.
    pub blockchain: Arc<Solana>,
    /// The solver engines.
    pub solvers: Vec<Solver>,
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
        // Propagate the OpenTelemetry trace context from incoming request
        // headers and record the trace id on the request span, so the
        // driver can correlate logs across services. `make_span` sets
        // the parent context and an empty `trace_id` field.
        // `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        // Global routes (healthz) live at the root.
        let mut app = Router::new().route("/healthz", get(routes::healthz));

        // Mount one router per solver engine under `/{solver_name}`.
        for solver in self.solvers {
            let solver_name = solver.name().to_owned();
            let competition = domain::Competition::new(solver);
            let state = State::new(self.blockchain.clone(), competition);

            let router = Router::new()
                .route("/solve", axum::routing::post(routes::solve))
                .route("/settle", axum::routing::post(routes::settle))
                .with_state(state);

            let path = format!("/{solver_name}");
            tracing::debug!(path = %path, "mounting solver");
            app = app.nest(&path, router);
        }

        let app = app
            // Disable the request body limit: solver payloads (auctions and solutions)
            // can exceed axum's 2MB default.
            .layer(DefaultBodyLimit::disable())
            .layer(RequestDecompressionLayer::new())
            .layer(tracing_layer);

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown.cancelled_owned())
            .await
    }
}

/// Shared state available to all route handlers for one solver engine.
#[derive(Clone)]
pub struct State(Arc<Inner>);

impl State {
    /// Build the shared state the handlers operate on.
    fn new(blockchain: Arc<Solana>, competition: domain::Competition) -> Self {
        Self(Arc::new(Inner {
            blockchain,
            competition,
        }))
    }

    /// The competition that runs auctions for this solver engine.
    fn competition(&self) -> &domain::Competition {
        &self.0.competition
    }

    /// The blockchain adapter, including the settlement program id.
    fn blockchain(&self) -> &Solana {
        &self.0.blockchain
    }
}

struct Inner {
    /// The shared Solana blockchain adapter.
    blockchain: Arc<Solana>,
    /// The competition that runs auctions for this solver engine.
    competition: domain::Competition,
}
