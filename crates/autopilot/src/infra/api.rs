use {
    crate::{
        infra::persistence::dto,
        solvable_orders::{DeltaAfterError, DeltaEvent, DeltaSubscribeError, SolvableOrdersCache},
    },
    alloy::primitives::Address,
    axum::{
        Router,
        body::Body,
        extract::{ConnectInfo, Path, Query, State as AxumState},
        http::{HeaderMap, StatusCode},
        response::{IntoResponse, Json, Response, sse},
        routing::get,
    },
    const_hex,
    futures::StreamExt,
    model::quote::NativeTokenPrice,
    observe::tracing::distributed::axum::{make_span, record_trace_id},
    price_estimation::{PriceEstimationError, native::NativePriceEstimating},
    prometheus::{IntCounter, IntGauge},
    serde::{Deserialize, Serialize},
    sha2::{Digest, Sha256},
    std::{
        convert::Infallible,
        net::SocketAddr,
        ops::RangeInclusive,
        sync::{Arc, OnceLock},
        time::{Duration, Instant},
    },
    subtle::ConstantTimeEq,
    tokio::sync::{broadcast::error::TryRecvError, oneshot},
    tokio_stream::{
        iter,
        wrappers::{BroadcastStream, ReceiverStream, errors::BroadcastStreamRecvError},
    },
};

/// Minimum allowed timeout for price estimation requests.
/// Values below this are not useful as they don't give estimators enough time.
const MIN_TIMEOUT: Duration = Duration::from_millis(250);
static DELTA_SYNC_API_KEY: OnceLock<Option<String>> = OnceLock::new();

#[cfg(test)]
static DELTA_SYNC_API_KEY_OVERRIDE: std::sync::LazyLock<std::sync::Mutex<Option<Option<String>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

#[derive(Clone)]
struct State {
    estimator: Arc<dyn NativePriceEstimating>,
    allowed_timeout: RangeInclusive<Duration>,
    solvable_orders_cache: Arc<SolvableOrdersCache>,
}

#[derive(Debug, Deserialize)]
struct NativePriceQuery {
    /// Optional timeout in milliseconds for the price estimation request.
    /// If not provided, uses the default timeout configured for autopilot.
    /// Values below 250ms are automatically clamped to the minimum (250ms).
    /// Values exceeding the configured maximum are clamped to the maximum.
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct DeltaStreamQuery {
    /// Return events strictly after this sequence.
    #[serde(default)]
    after_sequence: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaSnapshotResponse {
    version: u32,
    auction_id: u64,
    auction_sequence: u64,
    sequence: u64,
    oldest_available: u64,
    auction: dto::RawAuctionData,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaChecksumResponse {
    version: u32,
    sequence: u64,
    order_uid_hash: String,
    price_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaEventEnvelope {
    version: u32,
    auction_id: u64,
    auction_sequence: u64,
    from_sequence: u64,
    to_sequence: u64,
    // Wire-only field used to validate stream position against the replay snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_sequence: Option<u64>,
    published_at: String,
    events: Vec<DeltaEventDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeltaStreamGoneResponse {
    message: String,
    latest_sequence: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    oldest_available: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum DeltaEventDto {
    AuctionChanged {
        new_auction_id: u64,
    },
    OrderAdded {
        order: dto::order::Order,
    },
    OrderRemoved {
        uid: String,
    },
    OrderUpdated {
        order: dto::order::Order,
    },
    PriceChanged {
        token: Address,
        price: Option<String>,
    },
}

pub async fn serve(
    addr: SocketAddr,
    estimator: Arc<dyn NativePriceEstimating>,
    solvable_orders_cache: Arc<SolvableOrdersCache>,
    max_timeout: Duration,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), std::io::Error> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_with_listener(
        listener,
        estimator,
        solvable_orders_cache,
        max_timeout,
        shutdown,
    )
    .await
}

pub async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    estimator: Arc<dyn NativePriceEstimating>,
    solvable_orders_cache: Arc<SolvableOrdersCache>,
    max_timeout: Duration,
    shutdown: oneshot::Receiver<()>,
) -> Result<(), std::io::Error> {
    let state = State {
        estimator,
        allowed_timeout: MIN_TIMEOUT..=max_timeout,
        solvable_orders_cache,
    };
    let app = build_router(state);

    let addr = listener
        .local_addr()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 0)));
    tracing::info!(?addr, "serving HTTP API");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        shutdown.await.ok();
    })
    .await
}

fn build_router(state: State) -> Router {
    let mut app = Router::new().route("/native_price/{token}", get(get_native_price));
    if delta_sync_enabled() {
        app = app
            .route("/delta/snapshot", get(get_delta_snapshot))
            .route("/delta/stream", get(stream_delta_events))
            .route("/delta/checksum", get(get_delta_checksum));
    } else {
        tracing::warn!("delta sync API disabled via AUTOPILOT_DELTA_SYNC_ENABLED");
    }

    app.with_state(state).layer(
        tower::ServiceBuilder::new()
            .layer(tower_http::trace::TraceLayer::new_for_http().make_span_with(make_span))
            .map_request(record_trace_id),
    )
}

async fn get_delta_snapshot(headers: HeaderMap, AxumState(state): AxumState<State>) -> Response {
    if let Err(response) = authorize_delta_sync(&headers) {
        return response;
    }
    DeltaMetrics::get().snapshot_requests.inc();
    let Some(snapshot) = state.solvable_orders_cache.delta_snapshot().await else {
        DeltaMetrics::get().snapshot_empty.inc();
        return empty_delta_snapshot_response();
    };

    tracing::debug!(sequence = snapshot.sequence, "serving delta snapshot");

    let snapshot_bytes = match tokio::task::spawn_blocking(move || {
        let response = DeltaSnapshotResponse {
            version: 1,
            auction_id: snapshot.auction_id,
            auction_sequence: snapshot.auction_sequence,
            sequence: snapshot.sequence,
            oldest_available: snapshot.oldest_available,
            auction: dto::auction::from_domain(snapshot.auction),
        };
        serde_json::to_vec(&response)
    })
    .await
    {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(err)) => {
            tracing::error!(?err, "failed to serialize delta snapshot response");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
        Err(err) => {
            tracing::error!(?err, "failed to serialize delta snapshot response");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
    };

    let snapshot_len = snapshot_bytes.len();
    match Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(snapshot_bytes))
    {
        Ok(response) => {
            DeltaMetrics::get().snapshot_bytes.set(snapshot_len as i64);
            response
        }
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response(),
    }
}

async fn stream_delta_events(
    Query(query): Query<DeltaStreamQuery>,
    headers: HeaderMap,
    ConnectInfo(remote_addr): ConnectInfo<SocketAddr>,
    AxumState(state): AxumState<State>,
) -> Response {
    if let Err(response) = authorize_delta_sync(&headers) {
        return response;
    }
    DeltaMetrics::get().stream_connections.inc();
    let api_key_hash = api_key_hash(&headers);
    let active_stream = ActiveStreamGuard::new(remote_addr, api_key_hash.clone());
    // Subscribe while holding the cache lock used for replay so replay state and
    // receiver position remain consistent.
    let (mut receiver, replay) = match state
        .solvable_orders_cache
        .subscribe_deltas_with_replay_checked(query.after_sequence)
        .await
    {
        Ok(replay) => replay,
        Err(DeltaSubscribeError::MissingAfterSequence { .. }) => {
            return (
                StatusCode::BAD_REQUEST,
                "after_sequence is required; call /delta/snapshot and use its sequence",
            )
                .into_response();
        }
        Err(DeltaSubscribeError::DeltaAfter(err)) => match err {
            err @ DeltaAfterError::FutureSequence { .. } => {
                return delta_stream_after_error_response(err);
            }
            err @ DeltaAfterError::ResyncRequired { .. } => {
                DeltaMetrics::get().replay_miss.inc();
                return delta_stream_after_error_response(err);
            }
        },
    };
    let after_sequence = query.after_sequence.unwrap_or_default();
    let checkpoint_sequence = replay.checkpoint_sequence;
    let replay_envelopes = replay.envelopes;
    let max_stream_lag = delta_stream_max_lag();
    let baseline_sequence = after_sequence;
    let initial_lag = checkpoint_sequence.saturating_sub(after_sequence);
    if initial_lag > max_stream_lag {
        DeltaMetrics::get().stream_lagged.inc();
        tracing::warn!(
            after_sequence,
            checkpoint_sequence,
            initial_lag,
            "delta stream requires resnapshot due to lag"
        );
        return delta_stream_gone_response(
            format!("resnapshot required because subscription lagged by {initial_lag} messages"),
            checkpoint_sequence,
            None,
        );
    }

    let mut replay_payloads = Vec::new();
    let mut replay_to_sequence = checkpoint_sequence;
    for envelope in replay_envelopes {
        replay_to_sequence = replay_to_sequence.max(envelope.to_sequence);
        let envelope = to_api_envelope(envelope, Some(baseline_sequence));
        let payload = match serde_json::to_string(&envelope) {
            Ok(payload) => payload,
            Err(err) => {
                tracing::error!(?err, "failed to serialize delta envelope");
                DeltaMetrics::get().serialize_errors.inc();
                return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
            }
        };
        replay_payloads.push(payload);
    }

    let drained_outcome = match drain_live_envelopes(
        &mut receiver,
        checkpoint_sequence,
        baseline_sequence,
        &mut replay_to_sequence,
    ) {
        Ok(payloads) => payloads,
        Err(DrainError::Serialize(err)) => {
            tracing::error!(?err, "failed to serialize drained delta envelope");
            DeltaMetrics::get().serialize_errors.inc();
            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response();
        }
        Err(DrainError::Lagged(skipped)) => {
            DeltaMetrics::get().stream_lagged.inc();
            tracing::warn!(
                after_sequence,
                checkpoint_sequence,
                skipped,
                "delta stream lagged while draining live envelopes"
            );
            return delta_stream_gone_response(
                format!("resnapshot required because subscription lagged by {skipped} messages"),
                checkpoint_sequence,
                None,
            );
        }
    };
    replay_payloads.extend(drained_outcome.payloads);

    tracing::debug!(
        resume_from = after_sequence,
        checkpoint_sequence,
        replay_to_sequence,
        "delta stream replay served"
    );

    let replay_stream = iter(replay_payloads.into_iter().map(|payload| {
        Ok::<sse::Event, Infallible>(sse::Event::default().event("delta").data(payload))
    }));
    let replay_to_sequence = replay_to_sequence;
    let cache = Arc::clone(&state.solvable_orders_cache);
    let live_stream = BroadcastStream::new(receiver)
        .filter_map(move |item| {
            let cache = Arc::clone(&cache);
            async move {
                match item {
                    Ok(envelope) => {
                        // Avoid replay/live overlap if a message was delivered
                        // to the broadcast channel before the replay was built.
                        if envelope.to_sequence <= replay_to_sequence {
                            return None;
                        }

                        match serde_json::to_string(&to_api_envelope(
                            envelope,
                            Some(baseline_sequence),
                        )) {
                            Ok(payload) => Some(LiveEvent {
                                event: sse::Event::default().event("delta").data(payload),
                                close: false,
                            }),
                            Err(err) => {
                                tracing::error!(?err, "failed to serialize live delta envelope");
                                DeltaMetrics::get().serialize_errors.inc();
                                Some(LiveEvent {
                                    event: sse::Event::default()
                                        .event("error")
                                        .data("failed to serialize live delta envelope"),
                                    close: true,
                                })
                            }
                        }
                    }
                    Err(BroadcastStreamRecvError::Lagged(skipped)) => Some(LiveEvent {
                        event: sse::Event::default().event("resync_required").data({
                            DeltaMetrics::get().stream_lagged.inc();
                            let latest_sequence =
                                cache.delta_sequence().await.unwrap_or(checkpoint_sequence);
                            tracing::warn!(
                                after_sequence,
                                checkpoint_sequence,
                                skipped,
                                "delta stream lagged"
                            );
                            serde_json::json!({
                                "message": format!("delta stream lagged by {skipped} messages"),
                                "latestSequence": latest_sequence,
                                "skipped": skipped,
                            })
                            .to_string()
                        }),
                        close: true,
                    }),
                }
            }
        })
        .scan(false, |closed, item| {
            if *closed {
                return std::future::ready(None);
            }
            if item.close {
                *closed = true;
            }
            std::future::ready(Some(Ok::<sse::Event, Infallible>(item.event)))
        });
    let stream = replay_stream.chain(live_stream);

    let (stream_sender, stream_receiver) =
        tokio::sync::mpsc::channel::<Result<sse::Event, Infallible>>(delta_stream_buffer_size());
    let forward_sender = stream_sender.clone();
    let cache = Arc::clone(&state.solvable_orders_cache);
    tokio::spawn(async move {
        let _active_stream = active_stream;
        let mut stream = Box::pin(stream);
        loop {
            tokio::select! {
                _ = forward_sender.closed() => {
                    break;
                }
                item = stream.next() => {
                    let Some(item) = item else {
                        break;
                    };
                    match forward_sender.try_send(item) {
                        Ok(()) => {}
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            DeltaMetrics::get().stream_lagged.inc();
                            let latest_sequence =
                                cache.delta_sequence().await.unwrap_or(replay_to_sequence);
                            let resync_payload = serde_json::json!({
                                "message": "delta stream dropped slow consumer",
                                "latestSequence": latest_sequence,
                                "skipped": 0
                            })
                            .to_string();
                            let _ = forward_sender.try_send(Ok(
                                sse::Event::default()
                                    .event("resync_required")
                                    .data(resync_payload),
                            ));
                            tracing::warn!("delta stream dropped slow consumer");
                            break;
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                            break;
                        }
                    }
                }
            }
        }
    });

    if drained_outcome.closed {
        let latest_sequence = state
            .solvable_orders_cache
            .delta_sequence()
            .await
            .unwrap_or(checkpoint_sequence);
        let resync_payload = serde_json::json!({
            "message": "delta stream closed during replay drain",
            "latestSequence": latest_sequence,
            "skipped": 0
        })
        .to_string();
        let _ = stream_sender.try_send(Ok(sse::Event::default()
            .event("resync_required")
            .data(resync_payload)));
    }

    let stream = ReceiverStream::new(stream_receiver);

    sse::Sse::new(stream)
        .keep_alive(sse::KeepAlive::new().interval(delta_stream_keepalive_interval()))
        .into_response()
}

async fn get_delta_checksum(headers: HeaderMap, AxumState(state): AxumState<State>) -> Response {
    if let Err(response) = authorize_delta_sync(&headers) {
        return response;
    }

    let Some(checksum) = state.solvable_orders_cache.delta_checksum().await else {
        return StatusCode::NO_CONTENT.into_response();
    };

    Json(DeltaChecksumResponse {
        version: 1,
        sequence: checksum.sequence,
        order_uid_hash: checksum.order_uid_hash,
        price_hash: checksum.price_hash,
    })
    .into_response()
}

fn to_api_envelope(
    envelope: crate::solvable_orders::DeltaEnvelope,
    snapshot_sequence: Option<u64>,
) -> DeltaEventEnvelope {
    DeltaEventEnvelope {
        version: 1,
        auction_id: envelope.auction_id,
        auction_sequence: envelope.auction_sequence,
        from_sequence: envelope.from_sequence,
        to_sequence: envelope.to_sequence,
        snapshot_sequence,
        published_at: envelope.published_at.to_rfc3339(),
        events: envelope
            .events
            .into_iter()
            .map(delta_event_to_dto)
            .collect(),
    }
}

struct LiveEvent {
    event: sse::Event,
    close: bool,
}

#[derive(Debug)]
enum DrainError {
    Serialize(serde_json::Error),
    Lagged(u64),
}

#[derive(Debug)]
struct DrainOutcome {
    payloads: Vec<String>,
    closed: bool,
}

fn drain_live_envelopes(
    receiver: &mut tokio::sync::broadcast::Receiver<crate::solvable_orders::DeltaEnvelope>,
    checkpoint_sequence: u64,
    snapshot_sequence: u64,
    replay_to_sequence: &mut u64,
) -> Result<DrainOutcome, DrainError> {
    let mut drained = Vec::new();
    let mut closed = false;

    loop {
        match receiver.try_recv() {
            Ok(envelope) => {
                if envelope.to_sequence <= checkpoint_sequence {
                    continue;
                }
                *replay_to_sequence = envelope.to_sequence;
                let payload =
                    serde_json::to_string(&to_api_envelope(envelope, Some(snapshot_sequence)))
                        .map_err(DrainError::Serialize)?;
                drained.push(payload);
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Closed) => {
                closed = true;
                break;
            }
            Err(TryRecvError::Lagged(skipped)) => return Err(DrainError::Lagged(skipped)),
        }
    }

    Ok(DrainOutcome {
        payloads: drained,
        closed,
    })
}

fn empty_delta_snapshot_response() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

fn delta_stream_after_error_response(err: DeltaAfterError) -> Response {
    match err {
        DeltaAfterError::FutureSequence { latest } => (
            StatusCode::BAD_REQUEST,
            format!("afterSequence cannot be greater than latest sequence ({latest})"),
        )
            .into_response(),
        DeltaAfterError::ResyncRequired {
            oldest_available,
            latest,
        } => delta_stream_gone_response(
            "delta history does not include requested sequence; resnapshot required".to_string(),
            latest,
            Some(oldest_available),
        ),
    }
}

fn delta_stream_gone_response(
    message: String,
    latest_sequence: u64,
    oldest_available: Option<u64>,
) -> Response {
    let payload = DeltaStreamGoneResponse {
        message,
        latest_sequence,
        oldest_available,
    };
    (StatusCode::GONE, Json(payload)).into_response()
}

fn delta_sync_enabled() -> bool {
    shared::env::flag_enabled(
        std::env::var("AUTOPILOT_DELTA_SYNC_ENABLED")
            .ok()
            .as_deref(),
        false,
    )
}

fn delta_stream_max_lag() -> u64 {
    std::env::var("AUTOPILOT_DELTA_SYNC_STREAM_MAX_LAG")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(256)
}

fn delta_stream_buffer_size() -> usize {
    std::env::var("AUTOPILOT_DELTA_SYNC_STREAM_BUFFER")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(128)
}

fn delta_stream_keepalive_interval() -> Duration {
    let seconds = std::env::var("AUTOPILOT_DELTA_SYNC_STREAM_KEEPALIVE_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value| *value > 0)
        .unwrap_or(10);
    Duration::from_secs(seconds)
}

fn authorize_delta_sync(headers: &HeaderMap) -> Result<(), Response> {
    let Some(expected) = delta_sync_api_key() else {
        return Ok(());
    };

    let provided = headers
        .get("X-Delta-Sync-Api-Key")
        .and_then(|value| value.to_str().ok());
    if provided
        .map(|value| value.as_bytes().ct_eq(expected.as_bytes()))
        .map(bool::from)
        .unwrap_or(false)
    {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "Unauthorized").into_response())
    }
}

fn api_key_hash(headers: &HeaderMap) -> Option<String> {
    headers
        .get("X-Delta-Sync-Api-Key")
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            let mut hasher = Sha256::new();
            hasher.update(value.as_bytes());
            let digest = hasher.finalize();
            const_hex::encode(&digest[..8])
        })
}

fn delta_sync_api_key() -> Option<String> {
    #[cfg(test)]
    if let Some(value) = DELTA_SYNC_API_KEY_OVERRIDE
        .lock()
        .expect("delta sync api key override lock poisoned")
        .clone()
    {
        return value;
    }
    DELTA_SYNC_API_KEY
        .get_or_init(|| std::env::var("AUTOPILOT_DELTA_SYNC_API_KEY").ok())
        .clone()
}

#[cfg(test)]
fn set_delta_sync_api_key_override(value: Option<String>) {
    *DELTA_SYNC_API_KEY_OVERRIDE
        .lock()
        .expect("delta sync api key override lock poisoned") = Some(value);
}

#[cfg(test)]
fn clear_delta_sync_api_key_override() {
    *DELTA_SYNC_API_KEY_OVERRIDE
        .lock()
        .expect("delta sync api key override lock poisoned") = None;
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "delta_sync")]
struct DeltaMetrics {
    /// Total snapshot requests.
    snapshot_requests: IntCounter,
    /// Snapshot requests that found no active auction snapshot.
    snapshot_empty: IntCounter,
    /// Total stream connection attempts.
    stream_connections: IntCounter,
    /// Number of replay misses that require a new snapshot.
    replay_miss: IntCounter,
    /// Number of stream lag events where client fell behind.
    stream_lagged: IntCounter,
    /// Number of delta envelope serialization failures.
    serialize_errors: IntCounter,
    /// Size in bytes of the most recently served snapshot.
    snapshot_bytes: IntGauge,
    /// Currently active stream handlers.
    active_streams: IntGauge,
}

impl DeltaMetrics {
    fn get() -> &'static Self {
        DeltaMetrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}

struct ActiveStreamGuard {
    remote_addr: SocketAddr,
    api_key_hash: Option<String>,
}

impl ActiveStreamGuard {
    fn new(remote_addr: SocketAddr, api_key_hash: Option<String>) -> Self {
        DeltaMetrics::get().active_streams.inc();
        tracing::info!(
            remote_addr = %remote_addr,
            api_key_hash = api_key_hash.as_deref().unwrap_or("none"),
            "delta stream connected"
        );
        Self {
            remote_addr,
            api_key_hash,
        }
    }
}

impl Drop for ActiveStreamGuard {
    fn drop(&mut self) {
        DeltaMetrics::get().active_streams.dec();
        tracing::info!(
            remote_addr = %self.remote_addr,
            api_key_hash = self.api_key_hash.as_deref().unwrap_or("none"),
            "delta stream disconnected"
        );
    }
}

fn delta_event_to_dto(event: DeltaEvent) -> DeltaEventDto {
    match event {
        DeltaEvent::AuctionChanged { new_auction_id } => {
            DeltaEventDto::AuctionChanged { new_auction_id }
        }
        DeltaEvent::OrderAdded(order) => DeltaEventDto::OrderAdded {
            order: dto::order::from_domain(order),
        },
        DeltaEvent::OrderRemoved(uid) => DeltaEventDto::OrderRemoved {
            uid: uid.to_string(),
        },
        DeltaEvent::OrderUpdated(order) => DeltaEventDto::OrderUpdated {
            order: dto::order::from_domain(order),
        },
        DeltaEvent::PriceChanged { token, price } => DeltaEventDto::PriceChanged {
            token,
            price: price.map(|price| price.get().0.to_string()),
        },
    }
}

async fn get_native_price(
    Path(token): Path<Address>,
    Query(query): Query<NativePriceQuery>,
    AxumState(state): AxumState<State>,
) -> Response {
    let timeout = query
        .timeout_ms
        .map(Duration::from_millis)
        .unwrap_or(*state.allowed_timeout.end())
        .clamp(*state.allowed_timeout.start(), *state.allowed_timeout.end());

    let start = Instant::now();
    match state.estimator.estimate_native_price(token, timeout).await {
        Ok(price) => Json(NativeTokenPrice { price }).into_response(),
        Err(err) => {
            let elapsed = start.elapsed();
            tracing::warn!(
                ?err,
                ?token,
                ?timeout,
                ?elapsed,
                "failed to estimate native token price"
            );
            error_to_response(err)
        }
    }
}

fn error_to_response(err: PriceEstimationError) -> Response {
    match err {
        PriceEstimationError::NoLiquidity | PriceEstimationError::EstimatorInternal(_) => {
            (StatusCode::NOT_FOUND, "No liquidity").into_response()
        }
        PriceEstimationError::UnsupportedToken { token: _, reason } => (
            StatusCode::BAD_REQUEST,
            format!("Unsupported token, reason: {reason}"),
        )
            .into_response(),
        PriceEstimationError::RateLimited => {
            (StatusCode::TOO_MANY_REQUESTS, "Rate limited").into_response()
        }
        PriceEstimationError::UnsupportedOrderType(reason) => (
            StatusCode::BAD_REQUEST,
            format!("Unsupported order type, reason: {reason}"),
        )
            .into_response(),
        PriceEstimationError::ProtocolInternal(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal error").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            database::{Config as DbConfig, Postgres},
            domain,
            infra::Persistence,
            solvable_orders::DeltaEnvelope,
            test_helpers::test_order,
        },
        account_balances::BalanceFetching,
        axum::{
            body,
            http::{HeaderValue, Request},
        },
        bad_tokens::list_based::DenyListedTokens,
        bigdecimal::BigDecimal,
        chrono::Utc,
        cow_amm::Registry,
        database::byte_array::ByteArray,
        eth_domain_types as eth,
        ethrpc::{
            alloy::unbuffered_provider,
            block_stream::{BlockInfo, mock_single_block},
        },
        event_indexing::block_retriever::BlockRetriever,
        futures::FutureExt,
        price_estimation::{
            HEALTHY_PRICE_ESTIMATION_TIME,
            native::NativePriceEstimating,
            native_price_cache::{Cache, CachingNativePriceEstimator, NativePriceUpdater},
        },
        serde::Deserialize,
        serde_json,
        sqlx::postgres::PgPoolOptions,
        std::{collections::VecDeque, sync::Arc, time::Duration},
        tower::ServiceExt,
    };

    #[derive(Clone, Default)]
    struct StubBalanceFetcher;

    #[async_trait::async_trait]
    impl BalanceFetching for StubBalanceFetcher {
        async fn get_balances(
            &self,
            queries: &[account_balances::Query],
        ) -> Vec<anyhow::Result<alloy::primitives::U256>> {
            queries
                .iter()
                .map(|_| Ok(alloy::primitives::U256::ZERO))
                .collect()
        }

        async fn can_transfer(
            &self,
            _query: &account_balances::Query,
            _amount: alloy::primitives::U256,
        ) -> Result<(), account_balances::TransferSimulationError> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct StubNativePriceEstimator;

    impl NativePriceEstimating for StubNativePriceEstimator {
        fn estimate_native_price(
            &self,
            _token: Address,
            _timeout: Duration,
        ) -> futures::future::BoxFuture<'_, price_estimation::native::NativePriceEstimateResult>
        {
            async { Ok(1.0) }.boxed()
        }
    }

    fn test_price(value: u128) -> domain::auction::Price {
        domain::auction::Price::try_new(eth::Ether::from(eth::U256::from(value))).unwrap()
    }

    struct EnvGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, value);
            }
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            if let Some(value) = &self.previous {
                unsafe {
                    std::env::set_var(self.key, value);
                }
            } else {
                unsafe {
                    std::env::remove_var(self.key);
                }
            }
        }
    }

    struct ApiKeyOverrideGuard;

    impl ApiKeyOverrideGuard {
        fn set(value: Option<String>) -> Self {
            set_delta_sync_api_key_override(value);
            Self
        }
    }

    impl Drop for ApiKeyOverrideGuard {
        fn drop(&mut self) {
            clear_delta_sync_api_key_override();
        }
    }

    async fn test_cache() -> Arc<SolvableOrdersCache> {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgresql://")
            .expect("lazy pg pool");
        let postgres = Postgres {
            pool,
            config: DbConfig::default(),
        };
        let persistence = Persistence::new(None, Arc::new(postgres)).await;

        let balance_fetcher = Arc::new(StubBalanceFetcher::default());
        let deny_listed_tokens = DenyListedTokens::default();

        let native_price_estimator = StubNativePriceEstimator::default();
        let cache = Cache::new(Duration::from_secs(10), Default::default());
        let caching_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            cache,
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let native_price_estimator =
            NativePriceUpdater::new(caching_estimator, Duration::MAX, Default::default());

        let (provider, _wallet) = unbuffered_provider("http://localhost:0", None);
        let block_stream = mock_single_block(BlockInfo::default());
        let block_retriever = Arc::new(BlockRetriever {
            provider,
            block_stream,
        });
        let cow_amm_registry = Registry::new(block_retriever);

        let protocol_fees = domain::ProtocolFees::new(
            &configs::autopilot::fee_policy::FeePoliciesConfig::default(),
            Vec::new(),
            false,
        );

        SolvableOrdersCache::new(
            Duration::from_secs(0),
            persistence,
            order_validation::banned::Users::none(),
            balance_fetcher,
            deny_listed_tokens,
            native_price_estimator,
            Address::repeat_byte(0xEE),
            protocol_fees,
            cow_amm_registry,
            Duration::from_secs(1),
            Address::repeat_byte(0xFF),
            false,
        )
    }

    async fn db_cache() -> (Arc<SolvableOrdersCache>, Arc<Postgres>) {
        let postgres = Arc::new(Postgres::with_defaults().await.unwrap());
        let mut tx = postgres.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut tx).await.unwrap();
        tx.commit().await.unwrap();

        let persistence = Persistence::new(None, Arc::clone(&postgres)).await;

        let balance_fetcher = Arc::new(StubBalanceFetcher::default());
        let deny_listed_tokens = DenyListedTokens::default();

        let native_price_estimator = StubNativePriceEstimator::default();
        let cache = Cache::new(Duration::from_secs(10), Default::default());
        let caching_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            cache,
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let native_price_estimator =
            NativePriceUpdater::new(caching_estimator, Duration::MAX, Default::default());

        let (provider, _wallet) = unbuffered_provider("http://localhost:0", None);
        let block_stream = mock_single_block(BlockInfo::default());
        let block_retriever = Arc::new(BlockRetriever {
            provider,
            block_stream,
        });
        let cow_amm_registry = Registry::new(block_retriever);

        let protocol_fees = domain::ProtocolFees::new(
            &configs::autopilot::fee_policy::FeePoliciesConfig::default(),
            Vec::new(),
            false,
        );

        let cache = SolvableOrdersCache::new(
            Duration::from_secs(0),
            persistence,
            order_validation::banned::Users::none(),
            balance_fetcher,
            deny_listed_tokens,
            native_price_estimator,
            Address::repeat_byte(0xEE),
            protocol_fees,
            cow_amm_registry,
            Duration::from_secs(1),
            Address::repeat_byte(0xFF),
            true,
        );

        (cache, postgres)
    }

    fn apply_events(
        previous: domain::RawAuctionData,
        events: &[DeltaEvent],
    ) -> domain::RawAuctionData {
        let mut orders: std::collections::HashMap<domain::OrderUid, domain::Order> = previous
            .orders
            .into_iter()
            .map(|order| (order.uid, order))
            .collect();
        let mut prices = previous.prices;

        for event in events {
            match event {
                DeltaEvent::AuctionChanged { .. } => {}
                DeltaEvent::OrderAdded(order) | DeltaEvent::OrderUpdated(order) => {
                    orders.insert(order.uid, order.clone());
                }
                DeltaEvent::OrderRemoved(uid) => {
                    orders.remove(uid);
                }
                DeltaEvent::PriceChanged { token, price } => {
                    if let Some(price) = price {
                        prices.insert((*token).into(), *price);
                    } else {
                        prices.remove(&(*token).into());
                    }
                }
            }
        }

        let mut orders = orders.into_values().collect::<Vec<_>>();
        orders.sort_by_key(|order| order.uid.0);

        domain::RawAuctionData {
            block: previous.block,
            orders,
            prices,
            surplus_capturing_jit_order_owners: previous.surplus_capturing_jit_order_owners,
        }
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct WireEnvelope {
        version: u32,
        auction_id: u64,
        auction_sequence: u64,
        from_sequence: u64,
        to_sequence: u64,
        snapshot_sequence: Option<u64>,
        published_at: String,
        events: Vec<serde_json::Value>,
    }

    #[test]
    fn to_api_envelope_maps_core_fields() {
        let published_at = chrono::Utc::now();
        let envelope = crate::solvable_orders::DeltaEnvelope {
            auction_id: 7,
            auction_sequence: 3,
            from_sequence: 4,
            to_sequence: 5,
            published_at,
            published_at_instant: Instant::now(),
            events: vec![
                DeltaEvent::OrderAdded(test_order(1, 10)),
                DeltaEvent::OrderRemoved(domain::OrderUid([9; 56])),
                DeltaEvent::PriceChanged {
                    token: Address::repeat_byte(0xAB),
                    price: None,
                },
            ],
        };

        let dto = to_api_envelope(envelope, Some(42));
        assert_eq!(dto.version, 1);
        assert_eq!(dto.auction_id, 7);
        assert_eq!(dto.auction_sequence, 3);
        assert_eq!(dto.from_sequence, 4);
        assert_eq!(dto.to_sequence, 5);
        assert_eq!(dto.snapshot_sequence, Some(42));
        assert_eq!(dto.published_at, published_at.to_rfc3339());
        assert_eq!(dto.events.len(), 3);
    }

    #[test]
    fn api_envelope_serializes_with_expected_wire_shape() {
        let envelope = DeltaEventEnvelope {
            version: 1,
            auction_id: 9,
            auction_sequence: 4,
            from_sequence: 10,
            to_sequence: 11,
            snapshot_sequence: None,
            published_at: "2026-03-20T00:00:00Z".to_string(),
            events: vec![
                DeltaEventDto::OrderRemoved {
                    uid: "0xdeadbeef".to_string(),
                },
                DeltaEventDto::PriceChanged {
                    token: Address::repeat_byte(0xAA),
                    price: Some("123".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&envelope).unwrap();
        let wire: WireEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(wire.version, 1);
        assert_eq!(wire.auction_id, 9);
        assert_eq!(wire.auction_sequence, 4);
        assert_eq!(wire.from_sequence, 10);
        assert_eq!(wire.to_sequence, 11);
        assert_eq!(wire.snapshot_sequence, None);
        assert_eq!(wire.published_at, "2026-03-20T00:00:00Z");
        assert_eq!(wire.events.len(), 2);
    }

    #[test]
    fn delta_sync_enabled_parses_expected_values() {
        assert!(!shared::env::flag_enabled(None, false));
        assert!(!shared::env::flag_enabled(Some("false"), false));
        assert!(!shared::env::flag_enabled(Some("0"), false));
        assert!(shared::env::flag_enabled(Some("true"), false));
        assert!(shared::env::flag_enabled(Some("on"), false));
    }

    #[test]
    fn authorize_delta_sync_rejects_wrong_key() {
        let _guard = ApiKeyOverrideGuard::set(Some("expected".to_string()));
        let headers = HeaderMap::new();

        let response = authorize_delta_sync(&headers).unwrap_err();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn authorize_delta_sync_accepts_correct_key() {
        let _guard = ApiKeyOverrideGuard::set(Some("expected".to_string()));
        let mut headers = HeaderMap::new();
        headers.insert("X-Delta-Sync-Api-Key", HeaderValue::from_static("expected"));

        assert!(authorize_delta_sync(&headers).is_ok());
    }

    #[tokio::test]
    async fn delta_sync_disabled_disables_routes() {
        let _guard = EnvGuard::set("AUTOPILOT_DELTA_SYNC_ENABLED", "false");
        let cache = test_cache().await;
        let state = State {
            estimator: Arc::new(StubNativePriceEstimator::default()),
            allowed_timeout: MIN_TIMEOUT..=MIN_TIMEOUT,
            solvable_orders_cache: cache,
        };
        let app = build_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/delta/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn delta_snapshot_http_response_is_consistent_with_history() {
        let _guard = EnvGuard::set("AUTOPILOT_DELTA_SYNC_ENABLED", "true");
        let cache = test_cache().await;

        let token = Address::repeat_byte(0x11);
        let baseline = domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: std::collections::HashMap::from([(token.into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        };
        let events = vec![
            DeltaEvent::OrderUpdated(test_order(1, 11)),
            DeltaEvent::PriceChanged {
                token,
                price: Some(test_price(1200)),
            },
        ];
        let current = apply_events(baseline.clone(), &events);
        let envelope = DeltaEnvelope {
            auction_id: 1,
            auction_sequence: 2,
            from_sequence: 1,
            to_sequence: 2,
            published_at: chrono::Utc::now(),
            published_at_instant: Instant::now(),
            events,
        };
        cache
            .set_state_for_tests(current.clone(), 1, 2, 2, VecDeque::from([envelope]))
            .await;

        let state = State {
            estimator: Arc::new(StubNativePriceEstimator::default()),
            allowed_timeout: MIN_TIMEOUT..=MIN_TIMEOUT,
            solvable_orders_cache: cache,
        };
        let app = build_router(state);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/delta/snapshot")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

        assert_eq!(snapshot["sequence"], 2);
        assert_eq!(snapshot["auctionId"], 1);
        assert_eq!(snapshot["oldestAvailable"], 1);

        let expected = dto::auction::from_domain(current);
        assert_eq!(snapshot["auction"], serde_json::to_value(expected).unwrap());
    }

    #[tokio::test]
    #[ignore = "requires database-backed update pipeline"]
    async fn update_drives_snapshot_and_stream_end_to_end() {
        let _guard = EnvGuard::set("AUTOPILOT_DELTA_SYNC_ENABLED", "true");
        let (cache, postgres) = db_cache().await;

        let uid: database::OrderUid = ByteArray([0x11; 56]);
        let app_data: database::AppId = ByteArray([0x22; 32]);
        let now = Utc::now();

        let mut conn = postgres.pool.acquire().await.unwrap();
        database::app_data::insert(&mut conn, &app_data, b"{}")
            .await
            .unwrap();

        let order = database::orders::Order {
            uid,
            owner: ByteArray([0x33; 20]),
            creation_timestamp: now,
            sell_token: ByteArray([0x44; 20]),
            buy_token: ByteArray([0x55; 20]),
            receiver: None,
            sell_amount: BigDecimal::from(1000u64),
            buy_amount: BigDecimal::from(900u64),
            valid_to: now.timestamp() + 600,
            app_data,
            fee_amount: BigDecimal::from(0u64),
            kind: database::orders::OrderKind::Sell,
            partially_fillable: false,
            signature: vec![0u8; 65],
            signing_scheme: database::orders::SigningScheme::Eip712,
            settlement_contract: ByteArray([0x66; 20]),
            sell_token_balance: database::orders::SellTokenSource::Erc20,
            buy_token_balance: database::orders::BuyTokenDestination::Erc20,
            cancellation_timestamp: None,
            class: database::orders::OrderClass::Limit,
        };
        database::orders::insert_order(&mut conn, &order)
            .await
            .unwrap();
        drop(conn);

        cache.update(1, false).await.unwrap();

        let snapshot = cache
            .delta_snapshot()
            .await
            .expect("delta snapshot missing");
        let model_uid = model::order::OrderUid(order.uid.0);
        let snapshot_orders = dto::auction::from_domain(snapshot.auction).orders;
        let has_order = snapshot_orders.iter().any(|order| order.uid == model_uid);
        assert!(has_order);

        let (_receiver, replay) = cache
            .subscribe_deltas_with_replay_checked(Some(0))
            .await
            .expect("delta replay missing");
        let replay_has_order = replay.envelopes.iter().any(|envelope| {
            envelope.events.iter().any(|event| match event {
                DeltaEvent::OrderAdded(order) | DeltaEvent::OrderUpdated(order) => {
                    order.uid == domain::OrderUid(model_uid.0)
                }
                DeltaEvent::OrderRemoved(_) | DeltaEvent::AuctionChanged { .. } => false,
                DeltaEvent::PriceChanged { .. } => false,
            })
        });
        assert!(replay_has_order);
    }

    #[tokio::test]
    async fn slow_consumer_gets_resync_required_event() {
        let _guard_enabled = EnvGuard::set("AUTOPILOT_DELTA_SYNC_ENABLED", "true");
        let _guard_buffer = EnvGuard::set("AUTOPILOT_DELTA_SYNC_STREAM_BUFFER", "1");
        let cache = test_cache().await;
        cache
            .set_state_for_tests(
                domain::RawAuctionData {
                    block: 1,
                    orders: Vec::new(),
                    prices: std::collections::HashMap::new(),
                    surplus_capturing_jit_order_owners: Vec::new(),
                },
                0,
                0,
                0,
                VecDeque::new(),
            )
            .await;

        let state = State {
            estimator: Arc::new(StubNativePriceEstimator::default()),
            allowed_timeout: MIN_TIMEOUT..=MIN_TIMEOUT,
            solvable_orders_cache: Arc::clone(&cache),
        };
        let app = build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/delta/stream?after_sequence=0")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        for sequence in 1..=3u64 {
            cache
                .publish_delta_for_tests(DeltaEnvelope {
                    auction_id: 0,
                    auction_sequence: sequence,
                    from_sequence: sequence - 1,
                    to_sequence: sequence,
                    published_at: chrono::Utc::now(),
                    published_at_instant: Instant::now(),
                    events: vec![DeltaEvent::OrderAdded(test_order(sequence as u8, 10))],
                })
                .await;
        }

        let bytes = tokio::time::timeout(
            Duration::from_secs(2),
            body::to_bytes(response.into_body(), usize::MAX),
        )
        .await
        .expect("stream read timeout")
        .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();

        assert!(text.contains("event: resync_required"));
    }

    #[tokio::test]
    async fn publish_during_replay_is_drained_without_gap() {
        let (sender, mut receiver) = tokio::sync::broadcast::channel(8);
        let checkpoint_sequence = 5;
        let mut replay_to_sequence = checkpoint_sequence;

        // Simulates publication after replay checkpoint is captured but before
        // switching to live stream.
        sender
            .send(crate::solvable_orders::DeltaEnvelope {
                auction_id: 0,
                auction_sequence: 1,
                from_sequence: 5,
                to_sequence: 6,
                published_at: chrono::Utc::now(),
                published_at_instant: Instant::now(),
                events: vec![DeltaEvent::OrderAdded(test_order(1, 10))],
            })
            .unwrap();

        let drained = drain_live_envelopes(
            &mut receiver,
            checkpoint_sequence,
            checkpoint_sequence,
            &mut replay_to_sequence,
        )
        .unwrap();

        assert_eq!(drained.payloads.len(), 1);
        assert!(!drained.closed);
        assert_eq!(replay_to_sequence, 6);

        let wire: WireEnvelope = serde_json::from_str(&drained.payloads[0]).unwrap();
        assert_eq!(wire.from_sequence, 5);
        assert_eq!(wire.to_sequence, 6);
        assert_eq!(wire.events.len(), 1);
    }

    #[tokio::test]
    async fn drain_live_envelopes_ignores_pre_checkpoint_updates() {
        let (sender, mut receiver) = tokio::sync::broadcast::channel(8);
        let mut replay_to_sequence = 5;

        sender
            .send(crate::solvable_orders::DeltaEnvelope {
                auction_id: 0,
                auction_sequence: 1,
                from_sequence: 4,
                to_sequence: 5,
                published_at: chrono::Utc::now(),
                published_at_instant: Instant::now(),
                events: vec![DeltaEvent::OrderAdded(test_order(9, 90))],
            })
            .unwrap();
        sender
            .send(crate::solvable_orders::DeltaEnvelope {
                auction_id: 0,
                auction_sequence: 2,
                from_sequence: 5,
                to_sequence: 6,
                published_at: chrono::Utc::now(),
                published_at_instant: Instant::now(),
                events: vec![DeltaEvent::OrderUpdated(test_order(1, 11))],
            })
            .unwrap();

        let drained = drain_live_envelopes(&mut receiver, 5, 5, &mut replay_to_sequence).unwrap();

        assert_eq!(drained.payloads.len(), 1);
        assert!(!drained.closed);
        assert_eq!(replay_to_sequence, 6);
        let wire: WireEnvelope = serde_json::from_str(&drained.payloads[0]).unwrap();
        assert_eq!(wire.from_sequence, 5);
        assert_eq!(wire.to_sequence, 6);
    }

    #[tokio::test]
    async fn drain_live_envelopes_returns_lagged_when_receiver_fell_behind() {
        let (sender, mut receiver) = tokio::sync::broadcast::channel(1);
        let mut replay_to_sequence = 0;

        sender
            .send(crate::solvable_orders::DeltaEnvelope {
                auction_id: 0,
                auction_sequence: 1,
                from_sequence: 0,
                to_sequence: 1,
                published_at: chrono::Utc::now(),
                published_at_instant: Instant::now(),
                events: vec![DeltaEvent::OrderAdded(test_order(1, 10))],
            })
            .unwrap();
        sender
            .send(crate::solvable_orders::DeltaEnvelope {
                auction_id: 0,
                auction_sequence: 2,
                from_sequence: 1,
                to_sequence: 2,
                published_at: chrono::Utc::now(),
                published_at_instant: Instant::now(),
                events: vec![DeltaEvent::OrderAdded(test_order(2, 20))],
            })
            .unwrap();

        let err = drain_live_envelopes(&mut receiver, 0, 0, &mut replay_to_sequence).unwrap_err();
        assert!(matches!(err, DrainError::Lagged(_)));
    }

    #[tokio::test]
    async fn empty_delta_snapshot_response_has_expected_status_and_shape() {
        let response = empty_delta_snapshot_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        assert!(bytes.is_empty());
    }

    #[tokio::test]
    async fn delta_stream_future_sequence_error_maps_to_400_with_message() {
        let response =
            delta_stream_after_error_response(DeltaAfterError::FutureSequence { latest: 9 });
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("afterSequence cannot be greater than latest sequence (9)"));
    }

    #[tokio::test]
    async fn delta_stream_replay_miss_error_maps_to_410_with_message() {
        let response = delta_stream_after_error_response(DeltaAfterError::ResyncRequired {
            oldest_available: 5,
            latest: 12,
        });
        assert_eq!(response.status(), StatusCode::GONE);

        let bytes = body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("resnapshot required")
        );
        assert_eq!(payload["oldestAvailable"], 5);
        assert_eq!(payload["latestSequence"], 12);
    }
}
