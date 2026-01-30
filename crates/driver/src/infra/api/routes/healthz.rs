use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};

pub(in crate::infra::api) fn healthz(app: axum::Router<()>) -> axum::Router<()> {
    app.route("/healthz", get(route))
}

async fn route() -> Response {
    StatusCode::OK.into_response()
}
