pub async fn healthz() -> axum::http::StatusCode {
    axum::http::StatusCode::OK
}
