//! The readiness endpoint.

use {
    crate::infra::{
        api::{State, error},
        db,
    },
    axum::http::StatusCode,
};

/// Handle `GET /api/v1/ready`: 200 when the database answers, else 500.
pub async fn ready(state: axum::extract::State<State>) -> Result<StatusCode, error::Reply> {
    db::ping(state.pool()).await.map_err(|err| {
        tracing::error!(?err, "readiness check failed");
        error::reply(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", "")
    })?;
    Ok(StatusCode::OK)
}
