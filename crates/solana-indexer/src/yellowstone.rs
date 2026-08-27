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
        GeyserStream,
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
    use {
        super::*,
        futures::StreamExt,
        yellowstone_grpc_proto::{
            geyser::{
                CommitmentLevel,
                SubscribeRequest,
                SubscribeRequestFilterSlots,
                subscribe_update::UpdateOneof,
            },
            tonic::{Code, Status},
        },
    };

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

    /// A slot-status subscription with the reconnect wrapper off, so the node's
    /// own answers reach the caller. The library still injects its internal
    /// `BlockMeta` filter, so the stream carries more than slot statuses.
    async fn subscribe(url: &Url, from_slot: Option<u64>) -> GeyserStream {
        let x_token = std::env::var("YELLOWSTONE_X_TOKEN").ok();
        let mut builder = builder(url.clone(), x_token).unwrap();
        builder.reconnect_config = None;
        let mut client = builder.connect().await.unwrap();
        let request = SubscribeRequest {
            slots: [(
                "slots".to_owned(),
                SubscribeRequestFilterSlots {
                    filter_by_commitment: Some(true),
                    ..Default::default()
                },
            )]
            .into(),
            commitment: Some(CommitmentLevel::Confirmed as i32),
            from_slot,
            ..Default::default()
        };
        let (_sink, stream) = client.subscribe_with_request(Some(request)).await.unwrap();
        stream
    }

    async fn next_item(stream: &mut GeyserStream) -> Result<UpdateOneof, Status> {
        let update = tokio::time::timeout(Duration::from_secs(15), stream.next())
            .await
            .expect("node answered within 15s")
            .expect("stream open")?;
        Ok(update.update_oneof.expect("payload"))
    }

    /// The replay path only runs when `from_slot` is set: a live subscription
    /// streams at once, a recent `from_slot` is replayed, and one below the
    /// node's buffer is rejected before any data flows.
    #[tokio::test]
    #[ignore = "needs SOLANA_YELLOWSTONE_URL and, if the node requires one, YELLOWSTONE_X_TOKEN"]
    async fn node_rejects_only_replays_below_its_buffer() {
        let url = std::env::var("SOLANA_YELLOWSTONE_URL").expect("SOLANA_YELLOWSTONE_URL");
        let url = Url::parse(&url).unwrap();

        let mut live = subscribe(&url, None).await;
        let mut tip = None;
        for _ in 0..50 {
            if let UpdateOneof::Slot(slot) = next_item(&mut live).await.unwrap() {
                tip = Some(slot.slot);
                break;
            }
        }
        let tip = tip.expect("a slot status within the first 50 messages");
        println!("live subscription: streaming, tip {tip}");

        let recent = tip - 50;
        assert!(
            next_item(&mut subscribe(&url, Some(recent)).await)
                .await
                .is_ok()
        );
        println!("from_slot {recent}: replayed");

        let stale = tip - 100_000;
        let err = next_item(&mut subscribe(&url, Some(stale)).await)
            .await
            .unwrap_err();
        println!("from_slot {stale}: {} \"{}\"", err.code(), err.message());
        assert!(
            matches!(err.code(), Code::OutOfRange | Code::Internal),
            "{err}"
        );
    }
}
