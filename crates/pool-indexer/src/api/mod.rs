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

pub(super) fn resolve_chain_id(
    state: &AppState,
    network: &str,
) -> Result<u64, axum::response::Response> {
    state
        .resolve_network(network)
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())
}

pub(super) async fn latest_indexed_block(
    state: &AppState,
    chain_id: u64,
) -> Result<u64, axum::response::Response> {
    match crate::db::uniswap_v3::get_latest_indexed_block(&state.db, chain_id).await {
        Ok(Some(block_number)) => Ok(block_number),
        Ok(None) => Err(StatusCode::SERVICE_UNAVAILABLE.into_response()),
        Err(err) => Err(uniswap_v3::internal_error(err)),
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
            "/api/v1/{network}/uniswap/v3/pools/ticks",
            get(uniswap_v3::get_ticks_bulk),
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
