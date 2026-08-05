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
    /// Bind and serve until `shutdown` resolves.
    pub async fn serve(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> std::io::Result<()> {
        let app = Router::new()
            .route("/healthz", get(healthz))
            .route("/solve", post(solve))
            .with_state(self.dex)
            .layer(RequestBodyLimitLayer::new(REQUEST_BODY_LIMIT))
            .layer(axum::extract::DefaultBodyLimit::disable());

        let listener = tokio::net::TcpListener::bind(self.addr).await?;
        tracing::info!(addr = %self.addr, "solana-solvers listening");
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
