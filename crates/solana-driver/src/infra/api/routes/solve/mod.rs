use crate::infra::api::State;

pub async fn solve(_state: axum::extract::State<State>) -> axum::http::StatusCode {
    axum::http::StatusCode::NOT_IMPLEMENTED
}
