use axum::{http::StatusCode, response::IntoResponse};

pub async fn healthz() -> Response {
    StatusCode::OK
}
