pub(super) fn route(app: super::Router) -> super::Router {
    app.route("/", axum::routing::get(info))
}

async fn info() -> &'static str {
    "driver"
}
