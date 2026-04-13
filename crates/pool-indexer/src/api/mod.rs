pub mod uniswap_v3;

use {
    crate::config::NetworkName,
    axum::{Router, http::StatusCode, response::IntoResponse, routing::get},
    sqlx::PgPool,
    std::{collections::HashMap, sync::Arc},
    tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    tracing::Level,
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// Maps network name → chain_id for all configured networks.
    pub networks: HashMap<NetworkName, u64>,
}

impl AppState {
    pub fn resolve_network(&self, name: &str) -> Option<u64> {
        self.networks.get(&NetworkName::new(name)).copied()
    }
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route(
            "/api/v1/{network}/uniswap/v3/pools",
            get(uniswap_v3::get_pools),
        )
        .route(
            "/api/v1/{network}/uniswap/v3/pools/{pool_address}/ticks",
            get(uniswap_v3::get_ticks),
        )
        .with_state(state)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
}

async fn health() -> impl IntoResponse {
    StatusCode::OK
}
