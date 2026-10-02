#[test]
fn tls_uses_implicit_crypto_provider() {
    assert!(async_nats::rustls::crypto::CryptoProvider::get_default().is_none());
    let _ = async_nats::rustls::ClientConfig::builder();
}
