use crate::infra::api::State;

/// Stub handler.
pub async fn settle(_state: axum::extract::State<State>) -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_IMPLEMENTED
}
