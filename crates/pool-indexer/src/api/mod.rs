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
    /// The network this process indexes. Requests whose `{network}` path
    /// segment doesn't match get a 404.
    pub network: NetworkName,
}

impl AppState {
    pub fn is_network_configured(&self, name: &str) -> bool {
        self.network.as_str() == name
    }
}

/// Errors a handler can return. Input-shape errors (bad addresses, cursors,
/// too many ids) get rejected by the serde extractors and come back as
/// axum's default 400s before any handler runs.
#[derive(Debug)]
pub enum ApiError {
    /// `{network}` path segment doesn't match this process's network.
    NetworkNotFound,
    /// No checkpoint yet — indexer is still bootstrapping. 503 so clients
    /// retry instead of caching an empty response.
    NotReady,
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
