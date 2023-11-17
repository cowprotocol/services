use axum::{http::StatusCode, response::IntoResponse};

pub async fn healthz() -> impl IntoResponse {
    StatusCode::OK
}
