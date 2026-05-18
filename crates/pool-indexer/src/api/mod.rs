pub mod routes;
pub mod uniswap_v3;

pub use routes::router;
use {
    crate::config::NetworkName,
    axum::{
        Json,
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    sqlx::PgPool,
    std::collections::HashMap,
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
/// `Response` values. Input-shape errors (bad addresses, bad cursors, too
/// many ids) are handled earlier by the serde extractors and come back as
/// axum's default 400s — see [`crate::api::uniswap_v3::PoolIds`].
#[derive(Debug)]
pub enum ApiError {
    /// `{network}` path segment doesn't match any configured network. Says
    /// nothing about whether the network exists in the world, only that
    /// this indexer wasn't told about it.
    NetworkNotFound,
    /// The indexer has no checkpoint yet for this chain — it's still in
    /// bootstrap. Returned as 503 so clients retry rather than treat it
    /// as a permanent empty set.
    NotReady,
    /// The `after=` cursor didn't parse as a 20-byte hex address. Cursors
    /// are opaque but not arbitrary — clients must pass back exactly what
    /// the previous response returned.
    InvalidCursor,
    /// Unexpected failure inside the handler (usually DB). Body is generic
    /// 500; the underlying error is logged server-side.
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NetworkNotFound => StatusCode::NOT_FOUND.into_response(),
            Self::NotReady => StatusCode::SERVICE_UNAVAILABLE.into_response(),
            Self::InvalidCursor => bad_request("invalid cursor"),
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
