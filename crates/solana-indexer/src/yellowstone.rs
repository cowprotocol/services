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

/// First reconnect delay, doubled per attempt.
const RECONNECT_BACKOFF_INITIAL: Duration = Duration::from_millis(200);

/// Reconnect delay growth factor.
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
    // The builder default is `no_reconnect`, under which the `AutoReconnect`
    // wrapper gives up on the first stream error. Every stream drop gets a
    // fresh retry budget: ten doubling attempts from 200ms cover an outage of
    // a few minutes, anything longer ends the stream and the process restart
    // resumes from the last indexed slot.
    builder.reconnect_config = ReconnectConfig::default().with_backoff(Backoff::new(
        RECONNECT_BACKOFF_INITIAL,
        RECONNECT_BACKOFF_MULTIPLIER,
        RECONNECT_MAX_RETRIES,
    ));
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The TLS path is where a missing rustls crypto provider panics, and the
    /// retry assert catches losing the reconnect config (the builder default
    /// never reconnects).
    #[test]
    fn builder_configures_tls_and_reconnects() {
        let builder = builder(
            Url::parse("https://yellowstone.example.com:443").unwrap(),
            Some("secret".to_owned()),
        )
        .unwrap();
        assert!(builder.reconnect_config.backoff.max_retries > 0);
    }
}
