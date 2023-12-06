use prometheus::Encoder;

pub async fn metrics() -> String {
    let registry = observe::metrics::get_registry();
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&registry.gather(), &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap()
}
