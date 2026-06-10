pub mod routes;
pub mod uniswap_v3;

pub use routes::router;
use {
    crate::config::NetworkName,
    axum::{
        http::StatusCode,
        response::{IntoResponse, Response},
    },
    sqlx::PgPool,
};

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    /// The single network this process indexes. A URL whose `{network}`
    /// segment doesn't match yields a 404 — one process per network, one
    /// DB per network.
    pub network: NetworkName,
}

impl AppState {
    pub fn is_network_configured(&self, name: &str) -> bool {
        self.network.as_str() == name
    }
}

/// Structured error type for API handlers. Each variant decides its own HTTP
/// status via the `IntoResponse` impl so helpers can `?`-propagate failures
/// instead of handing around prebuilt `Response` values. Input-shape errors
/// (bad addresses, malformed cursors, too many ids) are rejected earlier by
/// the serde-driven extractors and come back as axum's default 400s —
/// [`crate::api::uniswap_v3::PoolIds`] is one such extractor.
#[derive(Debug)]
pub enum ApiError {
    /// `{network}` path segment doesn't match any configured network.
    NetworkNotFound,
    /// The indexer has no checkpoint yet for this chain — it's still in
    /// bootstrap. Returned as 503 so clients retry rather than treat it
    /// as a permanent empty set.
    NotReady,
    /// Unexpected failure inside the handler. Returns a generic error with a
    /// 500 status code; the underlying error is logged server-side.
    Internal(anyhow::Error),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::NetworkNotFound => StatusCode::NOT_FOUND.into_response(),
            Self::NotReady => StatusCode::SERVICE_UNAVAILABLE.into_response(),
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

pub(super) async fn latest_indexed_block(state: &AppState) -> Result<u64, ApiError> {
    crate::db::uniswap_v3::get_latest_indexed_block(&state.db)
        .await?
        .ok_or(ApiError::NotReady)
}
