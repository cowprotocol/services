use {
    axum::{http::StatusCode, response::IntoResponse},
};

pub async fn version_handler() -> impl IntoResponse {
    (StatusCode::OK, env!("VERGEN_GIT_DESCRIBE"))
}
