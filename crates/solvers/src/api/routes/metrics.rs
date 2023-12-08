pub async fn metrics() -> String {
    let registry = observe::metrics::get_registry();
    observe::metrics::encode(registry)
}
