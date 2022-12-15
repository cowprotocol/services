pub(super) fn route(app: axum::Router<super::State>) -> axum::Router<super::State> {
    app.route("/", axum::routing::get(info))
}

async fn info() -> &'static str {
    "driver"
}
