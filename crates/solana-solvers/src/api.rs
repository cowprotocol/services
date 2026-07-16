//! HTTP API for the solver engine.
//!
//! Serves the `/solve` contract the driver calls. At this stage the handler is
//! a scaffold: it accepts any auction and returns no solutions. Real quoting
//! and solution assembly land in later PRs.

use {
    crate::config::Config,
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
    pub config: Config,
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
            .with_state(Arc::new(self.config))
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

/// Scaffold `/solve`: accept any auction, return no solutions. The real solve
/// loop wraps each order's Jupiter quote into a single-order solution (later
/// PRs); until then the driver wiring can be exercised end to end against an
/// empty result.
async fn solve(State(_config): State<Arc<Config>>, Json(_auction): Json<Value>) -> Json<Value> {
    Json(json!({ "solutions": [] }))
}
