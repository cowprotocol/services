pub mod uniswap_v3;

use {
    axum::{Router, http::StatusCode, response::IntoResponse, routing::get},
    sqlx::PgPool,
    std::sync::Arc,
    tower_http::trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub chain_id: u64,
}

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/v1/uniswap/v3/pools", get(uniswap_v3::get_pools))
        .route(
            "/api/v1/uniswap/v3/pools/{pool_address}/ticks",
            get(uniswap_v3::get_ticks),
        )
        .with_state(state)
        .layer(TraceLayer::new_for_http())
}

async fn health() -> impl IntoResponse {
    StatusCode::OK
}
