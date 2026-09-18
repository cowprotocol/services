//! HTTP API server.

use {
    crate::{
        domain,
        infra::{blockchain::Solana, solver::Solver},
    },
    axum::{Router, extract::DefaultBodyLimit, routing::get},
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    std::{
        net::SocketAddr,
        num::NonZero,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    },
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
    /// Push reductions in ascending basis points; empty disables them.
    pub push_reduction_bps: Vec<u16>,
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
        // headers. Record the trace id on the request span to correlate logs
        // across services. `make_span` sets the parent context and an empty
        // `trace_id` field. `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        // Mount global routes (healthz) at the root.
        let mut app = Router::new().route("/healthz", get(routes::healthz));

        // Mount one router per solver engine under `/{solver_name}`.
        for solver in self.solvers {
            let solver_name = solver.name().to_owned();
            let solve_every_nth_auction = solver.solve_every_nth_auction();
            let competition = domain::Competition::new(
                solver,
                self.blockchain.clone(),
                self.push_reduction_bps.clone(),
            );
            let state = State::new(competition, solve_every_nth_auction);

            let router = Router::new()
                .route("/quote", axum::routing::post(routes::quote))
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
pub(crate) struct State(Arc<Inner>);

impl State {
    /// Build the shared state the handlers operate on.
    fn new(
        competition: domain::Competition,
        solve_every_nth_auction: Option<NonZero<u64>>,
    ) -> Self {
        Self(Arc::new(Inner {
            competition: Arc::new(competition),
            solve_every_nth_auction,
            solve_seq: AtomicU64::new(0),
        }))
    }

    /// The competition that runs auctions for this solver engine.
    fn competition(&self) -> &Arc<domain::Competition> {
        &self.0.competition
    }

    /// One in every N solves this solver takes part in, when throttled.
    pub(crate) fn solve_every_nth_auction(&self) -> Option<NonZero<u64>> {
        self.0.solve_every_nth_auction
    }

    /// The next solve's sequence number, counting every solve this driver
    /// receives. Sampling on this instead of the auction id keeps the stride
    /// correct however the auction id is derived.
    pub(crate) fn next_solve_seq(&self) -> u64 {
        self.0.solve_seq.fetch_add(1, Ordering::Relaxed)
    }
}

struct Inner {
    /// The competition that runs auctions for this solver engine.
    competition: Arc<domain::Competition>,
    /// One in every N solves this solver takes part in, when throttled.
    solve_every_nth_auction: Option<NonZero<u64>>,
    /// Monotonic count of solves received, for the stride sampling.
    solve_seq: AtomicU64,
}
