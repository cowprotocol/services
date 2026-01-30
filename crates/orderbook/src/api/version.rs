use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn version_handler() -> Response {
    (StatusCode::OK, env!("VERGEN_GIT_DESCRIBE")).into_response()
}
