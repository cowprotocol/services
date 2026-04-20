pub mod uniswap_v3;

use {
    crate::config::NetworkName,
    axum::{
        Json,
        Router,
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
    },
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

/// Structured error type for API handlers. Each variant decides its own HTTP
/// status + body via the `IntoResponse` impl so formatting lives in one place
/// and helpers can `?`-propagate failures instead of handing around prebuilt
/// `Response` values.
#[derive(Debug)]
pub enum ApiError {
    NetworkNotFound,
    NotReady,
    InvalidPoolId,
    InvalidPoolAddress,
    InvalidCursor,
    TooManyPoolIds { max: usize },
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NetworkNotFound => StatusCode::NOT_FOUND.into_response(),
            Self::NotReady => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Self::InvalidPoolId => bad_request("invalid pool id"),
            Self::InvalidPoolAddress => bad_request("invalid pool address"),
            Self::InvalidCursor => bad_request("invalid cursor"),
            Self::TooManyPoolIds { max } => bad_request(format!("too many pool ids; max {max}")),
            Self::Internal(err) => {
                tracing::error!(?err, "internal error");
                StatusCode::INTERNAL_SERVER_ERROR.into_response()
            }
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::Internal(err)
    }
}

fn bad_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": message.into() })),
    )
        .into_response()
}

pub(super) fn resolve_chain_id(state: &AppState, network: &str) -> Result<u64, ApiError> {
    state
        .resolve_network(network)
        .ok_or(ApiError::NetworkNotFound)
}

pub(super) async fn latest_indexed_block(state: &AppState, chain_id: u64) -> Result<u64, ApiError> {
    crate::db::uniswap_v3::get_latest_indexed_block(&state.db, chain_id)
        .await?
        .ok_or(ApiError::NotReady)
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
