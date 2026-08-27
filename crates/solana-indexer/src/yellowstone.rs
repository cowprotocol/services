//! Yellowstone gRPC client construction.
//!
//! The ingester neither reconnects nor answers server pings, so every client
//! built here has reconnect configured and HTTP/2 keepalive enabled.

use {
    std::time::Duration,
    url::Url,
    yellowstone_grpc_client::{
        Backoff,
        GeyserGrpcBuilder,
        GeyserGrpcBuilderError,
        GeyserGrpcClient,
        ReconnectConfig,
        ReconnectionPolicy,
    },
    yellowstone_grpc_proto::tonic::transport::ClientTlsConfig,
};

/// Deadline for establishing the TCP + TLS connection.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Deadline for a request to produce its response headers. Streams are
/// unaffected once they start delivering.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the transport sends HTTP/2 keepalive pings.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// First redial delay, doubled per attempt.
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(200);

/// Redial delay growth factor.
const RECONNECT_BACKOFF_MULTIPLIER: f64 = 2.0;

/// Dial attempts per outage, about 3.5 minutes in total. Exhausting them ends
/// the stream, the process restart resumes from the last indexed slot.
const RECONNECT_MAX_RETRIES: u32 = 10;

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
    // Reconnects continue from the live head rather than a checkpoint. A
    // checkpoint the provider has discarded is rejected as `Internal` while
    // the provider is fresh from a restart, and the wrapper retries it without
    // delay, so replaying across a reconnect is left to a backfill (BE-204).
    // The backoff covers dial failures only.
    builder.reconnect_config = Some(ReconnectConfig {
        backoff: Backoff::new(
            RECONNECT_BACKOFF_INITIAL,
            RECONNECT_BACKOFF_MULTIPLIER,
            RECONNECT_MAX_RETRIES,
        ),
        policy: ReconnectionPolicy::SkipMissedData,
    });
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The TLS path is where a missing rustls crypto provider panics, and the
    /// reconnect asserts catch losing the config or falling back to replaying
    /// a checkpoint.
    #[test]
    fn builder_configures_tls_and_reconnects_from_the_head() {
        let builder = builder(
            Url::parse("https://yellowstone.example.com:443").unwrap(),
            Some("secret".to_owned()),
        )
        .unwrap();
        let config = builder.reconnect_config.expect("reconnect config");
        assert!(config.backoff.max_retries > 0);
        assert!(matches!(config.policy, ReconnectionPolicy::SkipMissedData));
    }
}
