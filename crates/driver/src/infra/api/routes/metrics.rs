pub(in crate::infra::api) fn metrics(app: axum::Router<()>) -> axum::Router<()> {
    app.route("/metrics", axum::routing::get(route))
}

async fn route() -> String {
    let registry = observe::metrics::get_registry();
    observe::metrics::encode(registry)
}
