use crate::infra::api::State;

/// Stub handler.
pub async fn solve(_state: axum::extract::State<State>) -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_IMPLEMENTED
}
