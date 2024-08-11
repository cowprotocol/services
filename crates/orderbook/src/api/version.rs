use axum::{http::StatusCode, routing::MethodRouter};

const ENDPOINT: &str = "/api/v1/version";
async fn handler() -> (StatusCode, String) {
    (StatusCode::OK, env!("VERGEN_GIT_DESCRIBE").to_string())
}

pub fn route() -> (&'static str, MethodRouter<super::State>) {
    (ENDPOINT, axum::routing::get(handler))
}
