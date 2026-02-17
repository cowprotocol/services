use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub async fn healthz() -> Response {
    StatusCode::OK.into_response()
}
