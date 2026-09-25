//! Yellowstone gRPC client construction.
//!
//! Every client built here has HTTP/2 keepalive enabled, since the ingester
//! never answers server pings, and no reconnect layer: a stream error ends
//! the subscription, and the run loop backfills and resubscribes from the
//! persisted watermark, so no slot is skipped.

use {
    std::time::Duration,
    url::Url,
    yellowstone_grpc_client::{GeyserGrpcBuilder, GeyserGrpcBuilderError, GeyserGrpcClient},
    yellowstone_grpc_proto::tonic::transport::ClientTlsConfig,
};

/// Deadline for establishing the TCP + TLS connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Deadline for a request to produce its response headers. Streams are
/// unaffected once they start delivering.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the transport sends HTTP/2 keepalive pings.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// Cap on one decoded gRPC message, a limit, not an allocation. Our largest
/// message is a single transaction with meta, far below this, but a message
/// over the cap is a stream error that every reconnect replays, wedging the
/// indexer. Generous beats stuck.
const MAX_DECODING_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Connect to a Yellowstone gRPC endpoint.
///
/// `endpoint` decides TLS by scheme: `https` endpoints get TLS with the
/// system's native root certificates (the transport never infers TLS on its
/// own). `x_token` is the provider's authentication token, sent as the
/// `x-token` header on every request.
pub async fn connect(
    endpoint: Url,
    x_token: Option<String>,
) -> Result<GeyserGrpcClient, GeyserGrpcBuilderError> {
    builder(endpoint, x_token)?.connect().await
}

/// Assemble the configured builder without dialing.
fn builder(
    endpoint: Url,
    x_token: Option<String>,
) -> Result<GeyserGrpcBuilder, GeyserGrpcBuilderError> {
    let tls = endpoint.scheme() == "https";
    let endpoint = String::from(endpoint);
    let mut builder = GeyserGrpcBuilder::from_shared(endpoint)?
        .x_token(x_token)?
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .http2_keep_alive_interval(KEEP_ALIVE_INTERVAL)
        .keep_alive_while_idle(true)
        .max_decoding_message_size(MAX_DECODING_MESSAGE_SIZE);
    if tls {
        // rustls requires exactly one crypto provider, and the build graph
        // can enable several.
        let _ = rustls::crypto::ring::default_provider().install_default();
        builder = builder.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The TLS path is where a missing rustls crypto provider panics, and the
    /// reconnect assert catches the library's reconnect layer creeping back
    /// in: it resubscribes from the live head and skips the slots in between.
    #[test]
    fn builder_configures_tls_without_a_reconnect_layer() {
        let builder = builder(
            Url::parse("https://yellowstone.example.com:443").unwrap(),
            Some("secret".to_owned()),
        )
        .unwrap();
        assert!(builder.reconnect_config.is_none());
    }
}
