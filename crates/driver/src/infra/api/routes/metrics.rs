use prometheus::Encoder;

pub(in crate::infra::api) fn metrics(app: axum::Router<()>) -> axum::Router<()> {
    app.route("/metrics", axum::routing::get(route))
}

async fn route() -> String {
    let registry = global_metrics::get_metrics_registry();
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&registry.gather(), &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
