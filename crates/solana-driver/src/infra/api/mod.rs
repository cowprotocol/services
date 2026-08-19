//! HTTP API server.

use {
    crate::{
        domain,
        infra::{settlement::SettleOrder, solver},
    },
    axum::{
        Router,
        extract::DefaultBodyLimit,
        routing::{get, post},
    },
    cow_solana_rpc::SolanaRPC,
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer},
    std::{
        collections::HashMap,
        net::SocketAddr,
        sync::{Arc, Mutex},
    },
    tokio_util::sync::CancellationToken,
    tower::ServiceBuilder,
    tower_http::{decompression::RequestDecompressionLayer, trace::TraceLayer},
};

pub mod dto;
pub mod routes;

/// Solana's target slot time, the scale for slot-to-instant conversion.
const SLOT_DURATION: std::time::Duration = std::time::Duration::from_millis(400);

/// The Solana driver HTTP API server.
pub struct Api {
    /// Address the server binds to and listens on.
    pub addr: SocketAddr,
    /// The shared Solana RPC client.
    pub rpc: SolanaRPC,
    /// Configured solver engines.
    pub solvers: Vec<solver::Solver>,
    /// The solver identity settlements are signed with.
    pub keypair: Keypair,
    /// On-chain settlement program id.
    pub settlement_program: Pubkey,
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
        // record the trace id on the request span, so logs can be correlated across
        // services. `make_span` sets the parent context and an empty `trace_id` field;
        // `record_trace_id` then fills it in.
        let tracing_layer = ServiceBuilder::new()
            .layer(TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id);

        let state = State::new(
            self.rpc,
            self.solvers,
            self.keypair,
            self.settlement_program,
        );

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

/// One remembered `/solve` answer, the inputs `/settle` needs.
///
/// TODO: in-memory only, a restart forgets proposed solutions. Real solution
/// bookkeeping arrives with the settlement encoding work.
pub struct StoredSolution {
    pub orders: Vec<SettleOrder>,
    pub solution: domain::Solution,
}

impl State {
    /// Build the shared state the handlers operate on.
    fn new(
        rpc: SolanaRPC,
        solvers: Vec<solver::Solver>,
        keypair: Keypair,
        settlement_program: Pubkey,
    ) -> Self {
        Self(Arc::new(Inner {
            rpc,
            solvers,
            keypair,
            settlement_program,
            solutions: Mutex::new(HashMap::new()),
        }))
    }

    pub fn rpc(&self) -> &SolanaRPC {
        &self.0.rpc
    }

    pub fn solvers(&self) -> &[solver::Solver] {
        &self.0.solvers
    }

    pub fn keypair(&self) -> &Keypair {
        &self.0.keypair
    }

    /// Public key of the settlement signer, the identity reported to the
    /// autopilot.
    pub fn solver_identity(&self) -> Pubkey {
        self.0.keypair.pubkey()
    }

    pub fn settlement_program(&self) -> &Pubkey {
        &self.0.settlement_program
    }

    /// Convert the auction's deadline slot into an instant using the current
    /// slot and the target slot time.
    pub async fn deadline_from_slot(
        &self,
        deadline_slot: u64,
    ) -> Result<chrono::DateTime<chrono::Utc>, cow_solana_rpc::Error> {
        let current = self.0.rpc.slot().await?;
        let remaining = SLOT_DURATION
            * u32::try_from(deadline_slot.saturating_sub(current)).unwrap_or(u32::MAX);
        Ok(chrono::Utc::now() + chrono::Duration::from_std(remaining).expect("bounded duration"))
    }

    /// Remember the solutions of one auction under fresh ids and return them
    /// keyed as answered. Engine-local ids collide across engines, so the
    /// registry re-keys by position.
    pub fn store_solutions(
        &self,
        request: &dto::SolveRequest,
        solutions: Vec<domain::Solution>,
    ) -> Vec<(u64, domain::Solution)> {
        let mut registry = self.0.solutions.lock().unwrap();
        // A re-run of the same auction replaces its previous solutions.
        registry.retain(|(auction_id, _), _| *auction_id != request.id);
        solutions
            .into_iter()
            .enumerate()
            .map(|(index, solution)| {
                let solution_id = index as u64;
                registry.insert(
                    (request.id, solution_id),
                    Arc::new(StoredSolution {
                        orders: request.orders.iter().map(SettleOrder::from).collect(),
                        solution: solution.clone(),
                    }),
                );
                (solution_id, solution)
            })
            .collect()
    }

    /// The stored solution `/settle` references.
    pub fn stored_solution(
        &self,
        auction_id: i64,
        solution_id: u64,
    ) -> Option<Arc<StoredSolution>> {
        self.0
            .solutions
            .lock()
            .unwrap()
            .get(&(auction_id, solution_id))
            .cloned()
    }
}

struct Inner {
    /// The shared Solana RPC client.
    rpc: SolanaRPC,
    /// Configured solver engines.
    solvers: Vec<solver::Solver>,
    /// The solver identity settlements are signed with.
    keypair: Keypair,
    /// On-chain settlement program id.
    settlement_program: Pubkey,
    /// Proposed solutions by `(auction id, solution id)`, awaiting `/settle`.
    solutions: Mutex<HashMap<(i64, u64), Arc<StoredSolution>>>,
}
