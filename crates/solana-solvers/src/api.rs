//! HTTP API for the solver engine.
//!
//! Serves the `/solve` contract the driver calls.

use {
    crate::{dex::Dex, domain::solver, dto::auction::Auction},
    axum::{
        Json,
        Router,
        extract::State,
        routing::{get, post},
    },
    serde_json::{Value, json},
    std::{future::Future, net::SocketAddr, sync::Arc},
    tower_http::limit::RequestBodyLimitLayer,
};

const REQUEST_BODY_LIMIT: usize = 10 * 1024 * 1024;

pub struct Api {
    pub addr: SocketAddr,
    pub dex: Arc<Dex>,
}

impl Api {
    /// Bind to the configured address, returning the listener and the actual
    /// bound address (which differs from `addr` when binding to port 0).
    pub async fn bind(&self) -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        let local_addr = listener.local_addr()?;
        tracing::info!(addr = %local_addr, "solana-solvers listening");
        Ok((listener, local_addr))
    }

    /// Serve the API on the given listener until `shutdown` resolves.
    pub async fn serve(
        self,
        listener: tokio::net::TcpListener,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let app = Router::new()
            .route("/healthz", get(healthz))
            .route("/solve", post(solve))
            .with_state(self.dex)
            .layer(RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT))
            .layer(axum::extract::DefaultBodyLimit::disable());

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown)
            .await
    }
}

async fn healthz() -> &'static str {
    "ok"
}

/// Quote every order in the auction and return the single-order solutions.
async fn solve(State(dex): State<Arc<Dex>>, Json(auction): Json<Auction>) -> Json<Value> {
    let solutions = solver::solve(dex.as_ref(), &auction).await;
    Json(json!({ "solutions": solutions }))
}
