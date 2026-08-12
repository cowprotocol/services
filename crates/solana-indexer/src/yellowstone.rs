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

/// Upper bound on a single decoded gRPC message. Transaction updates carry
/// the full transaction plus its meta (logs, balances, inner instructions),
/// so the tonic default of 4 MiB is too tight for pathological cases.
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
        // rustls needs exactly one process-level crypto provider, and the
        // build graph can enable several. Installing one explicitly keeps
        // sibling crates' feature choices from breaking the TLS setup.
        let _ = rustls::crypto::ring::default_provider().install_default();
        builder = builder.tls_config(ClientTlsConfig::new().with_native_roots())?;
    }
    // The builder default is `no_reconnect`, under which the `AutoReconnect`
    // wrapper gives up on the first stream error. Every stream drop gets a
    // fresh retry budget: ten doubling attempts from 200ms cover an outage of
    // a few minutes, anything longer ends the stream and the process restart
    // resumes from the last indexed slot.
    builder.reconnect_config =
        ReconnectConfig::default().with_backoff(Backoff::new(Duration::from_millis(200), 2.0, 10));
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    #[test]
    fn builds_with_tls_and_token() {
        let builder = builder(
            url("https://yellowstone.example.com:443"),
            Some("secret".to_owned()),
        )
        .unwrap();
        assert!(builder.reconnect_config.backoff.max_retries > 0);
    }

    #[test]
    fn builds_plaintext_without_token() {
        builder(url("http://localhost:10000"), None).unwrap();
    }

    #[test]
    fn rejects_malformed_token() {
        builder(url("http://localhost:10000"), Some("tok\nen".to_owned())).unwrap_err();
    }
}
