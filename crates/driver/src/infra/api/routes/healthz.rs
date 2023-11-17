use axum::{http::StatusCode, response::IntoResponse, routing::get};

pub(in crate::infra::api) fn healthz(app: axum::Router<()>) -> axum::Router<()> {
    app.route("/healthz", get(route))
}

async fn route() -> impl IntoResponse {
    StatusCode::OK
}
