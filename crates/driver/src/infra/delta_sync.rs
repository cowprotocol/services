#[cfg(any(test, feature = "test-helpers"))]
use std::sync::Mutex as StdMutex;
#[cfg(test)]
use std::sync::atomic::{AtomicU8, Ordering};

use {
    crate::{
        domain::competition::delta_replica::{
            Envelope,
            Replica,
            ReplicaChecksum,
            ReplicaState,
            Snapshot,
        },
        infra::observe::metrics,
    },
    reqwest::{StatusCode, Url},
    serde::{Deserialize, Serialize},
    std::{
        collections::HashMap,
        sync::{Arc, LazyLock, OnceLock, RwLock as StdRwLock},
        time::{Duration, Instant},
    },
    tokio::sync::{Mutex, RwLock},
};

const DEFAULT_RETRY_DELAY: Duration = Duration::from_secs(2);
const STREAM_RETRY_BACKOFF: Duration = Duration::from_millis(500);
const SNAPSHOT_BOOTSTRAP_RETRIES: usize = 3;
const SNAPSHOT_BOOTSTRAP_DELAY: Duration = Duration::from_millis(100);
const DELTA_SYNC_API_KEY_HEADER: &str = "X-Delta-Sync-Api-Key";
static DELTA_REPLICA: LazyLock<StdRwLock<Arc<RwLock<Replica>>>> =
    LazyLock::new(|| StdRwLock::new(Arc::new(RwLock::new(Replica::default()))));
#[cfg(any(test, feature = "test-helpers"))]
static DELTA_REPLICA_TEST_MUTEX: LazyLock<StdMutex<()>> = LazyLock::new(|| StdMutex::new(()));
static DELTA_CHECKSUM_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to create reqwest client for delta checksum")
});
static REPLICA_PREPROCESSING_ENABLED: OnceLock<bool> = OnceLock::new();
static DELTA_STREAM_GONE_RETRY_THRESHOLD: OnceLock<u64> = OnceLock::new();
static DELTA_REPLICA_MAX_STALENESS: OnceLock<Option<Duration>> = OnceLock::new();
static DELTA_REPLICA_RESNAPSHOT_INTERVAL: OnceLock<Option<Duration>> = OnceLock::new();
static BOOTSTRAP_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
#[cfg(test)]
static REPLICA_PREPROCESSING_OVERRIDE: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Clone)]
pub(crate) struct ReplicaSnapshot {
    pub(crate) auction_id: u64,
    pub(crate) sequence: u64,
    pub(crate) orders: Vec<crate::infra::api::routes::solve::dto::solve_request::Order>,
    pub(crate) prices: HashMap<alloy::primitives::Address, String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplicaHealth {
    pub state: ReplicaState,
    pub sequence: u64,
    pub order_count: usize,
    pub last_update: Option<chrono::DateTime<chrono::Utc>>,
    pub last_update_age_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaStreamGonePayload {
    latest_sequence: u64,
    #[serde(default)]
    oldest_available: Option<u64>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaChecksumResponse {
    sequence: u64,
    order_uid_hash: String,
    price_hash: String,
}

/// Starts a background delta-sync task if DRIVER_DELTA_SYNC_AUTOPILOT_URL is
/// set.
///
/// The task keeps a local replica up to date from snapshot + delta SSE stream.
pub fn maybe_spawn_from_env() -> Option<tokio::task::JoinHandle<()>> {
    if !shared::env::flag_enabled(
        std::env::var("DRIVER_DELTA_SYNC_ENABLED").ok().as_deref(),
        true,
    ) {
        tracing::warn!("driver delta sync disabled via DRIVER_DELTA_SYNC_ENABLED");
        return None;
    }

    let base = delta_sync_base_url_from_env()?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to create reqwest client for delta sync");

    let replica = delta_replica();

    Some(tokio::spawn(async move {
        match run(client, base, replica).await {
            Ok(()) => tracing::warn!("delta sync task exited without error"),
            Err(err) => tracing::error!(?err, "delta sync task exited"),
        }
    }))
}

pub(crate) async fn snapshot() -> Option<ReplicaSnapshot> {
    let replica = delta_replica();
    let replica = replica.read().await;
    snapshot_from_replica(&replica)
}

/// Reset the global delta replica (intended for test isolation).
#[cfg(any(test, feature = "test-helpers"))]
pub fn reset_delta_replica_for_tests() {
    let mut lock = DELTA_REPLICA
        .write()
        .expect("delta replica lock poisoned during reset");
    *lock = Arc::new(RwLock::new(Replica::default()));
}

#[cfg(test)]
pub(crate) async fn set_replica_snapshot_for_tests(snapshot: Snapshot) {
    let replica = delta_replica();
    let mut lock = replica.write().await;
    lock.set_state(ReplicaState::Syncing);
    lock.apply_snapshot(snapshot)
        .expect("failed to apply test snapshot to replica");
}

/// Global test guard to serialize replica usage across tests.
#[cfg(any(test, feature = "test-helpers"))]
pub struct DeltaReplicaTestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(any(test, feature = "test-helpers"))]
impl DeltaReplicaTestGuard {
    pub fn acquire() -> Self {
        let lock = DELTA_REPLICA_TEST_MUTEX
            .lock()
            .expect("delta replica test mutex poisoned");
        reset_delta_replica_for_tests();
        Self { _lock: lock }
    }
}

#[cfg(any(test, feature = "test-helpers"))]
impl Drop for DeltaReplicaTestGuard {
    fn drop(&mut self) {
        reset_delta_replica_for_tests();
    }
}

fn snapshot_from_replica(replica: &Replica) -> Option<ReplicaSnapshot> {
    if !matches!(replica.state(), ReplicaState::Ready) {
        return None;
    }
    Some(ReplicaSnapshot {
        auction_id: replica.auction_id(),
        sequence: replica.sequence(),
        orders: replica.orders().values().cloned().collect(),
        prices: replica.prices().clone(),
    })
}

pub async fn replica_health() -> Option<ReplicaHealth> {
    let checksum_enabled = delta_sync_checksum_enabled();
    let base_url = if checksum_enabled {
        delta_sync_base_url_from_env()
    } else {
        None
    };

    let replica = delta_replica();
    let replica = replica.read().await;
    let now = chrono::Utc::now();
    let last_update_age_seconds = replica
        .last_update()
        .map(|timestamp| now.signed_duration_since(timestamp).num_seconds())
        .and_then(|seconds| u64::try_from(seconds).ok());

    let local_checksum = base_url.as_ref().map(|_| replica.checksum());

    if let Some(age) = last_update_age_seconds {
        metrics::get()
            .delta_replica_last_update_age_seconds
            .set(age as i64);
    }

    let health = ReplicaHealth {
        state: replica.state(),
        sequence: replica.sequence(),
        order_count: replica.orders().len(),
        last_update: replica.last_update(),
        last_update_age_seconds,
    };
    drop(replica);

    if let (Some(base_url), Some(local_checksum)) = (base_url, local_checksum) {
        if let Err(err) = compare_replica_checksum(&base_url, local_checksum).await {
            tracing::warn!(?err, "delta replica checksum comparison failed");
        }
    }

    Some(health)
}

pub async fn replica_state() -> Option<ReplicaState> {
    Some(delta_replica().read().await.state())
}

pub async fn replica_is_fresh() -> Option<bool> {
    let Some(max_staleness) = delta_replica_max_staleness() else {
        return Some(true);
    };
    let replica = delta_replica();
    let replica = replica.read().await;
    let Some(last_update) = replica.last_update() else {
        return Some(false);
    };
    let now = chrono::Utc::now();
    let age = now.signed_duration_since(last_update).to_std().ok()?;
    Some(age <= max_staleness)
}

pub async fn ensure_replica_snapshot_from_env() -> anyhow::Result<bool> {
    if !shared::env::flag_enabled(
        std::env::var("DRIVER_DELTA_SYNC_ENABLED").ok().as_deref(),
        true,
    ) {
        return Ok(false);
    }

    let Some(base_url) = delta_sync_base_url_from_env() else {
        return Ok(false);
    };

    let replica = delta_replica();

    let _bootstrap_guard = BOOTSTRAP_LOCK.lock().await;

    {
        let current = replica.read().await;
        if matches!(current.state(), ReplicaState::Ready) {
            return Ok(true);
        }
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("failed to create reqwest client for delta sync bootstrap");

    let mut snapshot = None;
    let mut last_err = None;
    for attempt in 0..SNAPSHOT_BOOTSTRAP_RETRIES {
        match fetch_snapshot(&client, &base_url).await {
            Ok(payload) => {
                snapshot = Some(payload);
                break;
            }
            Err(err) => {
                last_err = Some(err);
                if attempt + 1 < SNAPSHOT_BOOTSTRAP_RETRIES {
                    tokio::time::sleep(SNAPSHOT_BOOTSTRAP_DELAY).await;
                }
            }
        }
    }
    let snapshot = snapshot
        .ok_or_else(|| last_err.unwrap_or_else(|| anyhow::anyhow!("snapshot bootstrap failed")))?;
    {
        let mut lock = replica.write().await;
        lock.set_state(ReplicaState::Syncing);
        lock.apply_snapshot(snapshot)?;
    }
    Ok(true)
}

pub fn replica_preprocessing_enabled() -> bool {
    #[cfg(test)]
    if let Some(value) = replica_preprocessing_override() {
        return value;
    }
    *REPLICA_PREPROCESSING_ENABLED.get_or_init(|| {
        shared::env::flag_enabled(
            std::env::var("DRIVER_DELTA_SYNC_USE_REPLICA")
                .ok()
                .as_deref(),
            false,
        )
    })
}

#[cfg(test)]
fn replica_preprocessing_override() -> Option<bool> {
    match REPLICA_PREPROCESSING_OVERRIDE.load(Ordering::SeqCst) {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) fn set_replica_preprocessing_override(value: Option<bool>) {
    let encoded = match value {
        Some(true) => 1,
        Some(false) => 2,
        None => 0,
    };
    REPLICA_PREPROCESSING_OVERRIDE.store(encoded, Ordering::SeqCst);
}

async fn run(
    client: reqwest::Client,
    base_url: Url,
    replica: Arc<RwLock<Replica>>,
) -> anyhow::Result<()> {
    loop {
        {
            let mut lock = replica.write().await;
            lock.set_state(ReplicaState::Syncing);
        }
        match fetch_snapshot(&client, &base_url).await {
            Ok(snapshot) => {
                let applied = {
                    let mut lock = replica.write().await;
                    lock.apply_snapshot(snapshot)
                };
                if let Err(err) = applied {
                    tracing::warn!(?err, "delta sync snapshot apply failed; retrying");
                    tokio::time::sleep(DEFAULT_RETRY_DELAY).await;
                    continue;
                }
                let view = replica.read().await;
                tracing::info!(
                    sequence = view.sequence(),
                    orders = view.orders().len(),
                    prices = view.prices().len(),
                    "delta sync snapshot applied"
                );
            }
            Err(err) => {
                {
                    let mut lock = replica.write().await;
                    lock.set_state(ReplicaState::Resyncing);
                }
                tracing::warn!(?err, "delta sync snapshot fetch failed; retrying");
                tokio::time::sleep(DEFAULT_RETRY_DELAY).await;
                continue;
            }
        }

        let snapshot_started_at = Instant::now();
        loop {
            match follow_stream(&client, &base_url, &replica, snapshot_started_at).await {
                Ok(StreamControl::Resnapshot) => {
                    {
                        let mut lock = replica.write().await;
                        lock.set_state(ReplicaState::Resyncing);
                    }
                    let view = replica.read().await;
                    tracing::warn!(
                        sequence = view.sequence(),
                        "delta stream requested resnapshot"
                    );
                    break;
                }
                Ok(StreamControl::RetryStream) => {
                    tracing::warn!("delta stream requested retry without resnapshot");
                    tokio::time::sleep(STREAM_RETRY_BACKOFF).await;
                }
                Err(err) => {
                    {
                        let mut lock = replica.write().await;
                        lock.set_state(ReplicaState::Resyncing);
                    }
                    let view = replica.read().await;
                    tracing::warn!(
                        ?err,
                        sequence = view.sequence(),
                        "delta stream failed; forcing resnapshot"
                    );
                    break;
                }
            }
        }

        tokio::time::sleep(DEFAULT_RETRY_DELAY).await;
    }
}

async fn fetch_snapshot(client: &reqwest::Client, base_url: &Url) -> anyhow::Result<Snapshot> {
    let url = shared::url::join(base_url, "delta/snapshot");
    let response = apply_delta_sync_auth(client.get(url)).send().await?;
    if response.status() == StatusCode::NO_CONTENT {
        anyhow::bail!("delta snapshot unavailable")
    }
    let response = response.error_for_status()?;
    Ok(response.json::<Snapshot>().await?)
}

#[derive(Clone, Copy, Debug)]
enum StreamControl {
    Resnapshot,
    RetryStream,
}

async fn follow_stream(
    client: &reqwest::Client,
    base_url: &Url,
    replica: &std::sync::Arc<RwLock<Replica>>,
    snapshot_started_at: Instant,
) -> anyhow::Result<StreamControl> {
    let url = shared::url::join(base_url, "delta/stream");
    let after_sequence = replica.read().await.sequence();
    let response = apply_delta_sync_auth(client.get(url))
        .query(&[("after_sequence", after_sequence)])
        .send()
        .await?;
    if response.status() == StatusCode::GONE {
        let payload = response.json::<DeltaStreamGonePayload>().await.ok();
        if let Some(payload) = payload {
            if payload
                .oldest_available
                .is_some_and(|oldest| after_sequence < oldest)
            {
                tracing::warn!(
                    after_sequence,
                    latest_sequence = payload.latest_sequence,
                    oldest_available = ?payload.oldest_available,
                    message = payload.message.as_deref().unwrap_or(""),
                    "delta stream lagged beyond retention; resnapshot required"
                );
                return Ok(StreamControl::Resnapshot);
            }
            let lag = payload.latest_sequence.saturating_sub(after_sequence);
            if lag <= delta_stream_gone_retry_threshold() {
                tracing::warn!(
                    after_sequence,
                    latest_sequence = payload.latest_sequence,
                    oldest_available = ?payload.oldest_available,
                    message = payload.message.as_deref().unwrap_or(""),
                    "delta stream lagged but within retry threshold"
                );
                return Ok(StreamControl::RetryStream);
            }
        }
        return Ok(StreamControl::Resnapshot);
    }

    let mut response = response.error_for_status()?;
    let mut buffer = String::new();
    let max_staleness = delta_replica_max_staleness();
    let resnapshot_interval = delta_replica_resnapshot_interval();

    loop {
        tokio::select! {
            chunk = response.chunk() => {
                match chunk? {
                    Some(bytes) => {
                        let chunk = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
                        buffer.push_str(&chunk);

                        while let Some(idx) = buffer.find("\n\n") {
                            let block = buffer[..idx].to_string();
                            buffer.drain(..idx + 2);

                            match handle_sse_block(&block, replica).await? {
                                BlockControl::Continue => {}
                                BlockControl::Resnapshot => return Ok(StreamControl::Resnapshot),
                            }
                        }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                if let Some(max_staleness) = max_staleness {
                    let last_update = replica.read().await.last_update();
                    let now = chrono::Utc::now();
                    if let Some(last_update) = last_update {
                        if let Ok(age) = now.signed_duration_since(last_update).to_std() {
                            if age > max_staleness {
                                tracing::warn!(?age, "delta replica update age exceeded max staleness");
                                return Ok(StreamControl::Resnapshot);
                            }
                        }
                    }
                }

                if let Some(interval) = resnapshot_interval {
                    if snapshot_started_at.elapsed() > interval {
                        tracing::warn!("delta replica resnapshot interval elapsed");
                        return Ok(StreamControl::Resnapshot);
                    }
                }
            }
        }
    }

    anyhow::bail!("delta stream closed")
}

#[derive(Clone, Copy, Debug)]
enum BlockControl {
    Continue,
    Resnapshot,
}

async fn handle_sse_block(
    block: &str,
    replica: &std::sync::Arc<RwLock<Replica>>,
) -> anyhow::Result<BlockControl> {
    let (event, data) = parse_sse_block(block);
    let Some(data) = data else {
        return Ok(BlockControl::Continue);
    };

    match event {
        "delta" => {
            let envelope: Envelope = serde_json::from_str(&data)?;
            let applied = {
                let mut lock = replica.write().await;
                lock.apply_delta(envelope)
            };
            if let Err(err) = applied {
                tracing::warn!(?err, "delta envelope apply failed; resnapshot required");
                return Ok(BlockControl::Resnapshot);
            }
            let view = replica.read().await;
            tracing::debug!(
                sequence = view.sequence(),
                orders = view.orders().len(),
                prices = view.prices().len(),
                "delta envelope applied"
            );
            Ok(BlockControl::Continue)
        }
        "resync_required" => Ok(BlockControl::Resnapshot),
        "error" => {
            tracing::warn!(payload = %data, "delta stream returned error event");
            Ok(BlockControl::Resnapshot)
        }
        _ => Ok(BlockControl::Continue),
    }
}

fn delta_sync_base_url_from_env() -> Option<Url> {
    let url = std::env::var("DRIVER_DELTA_SYNC_AUTOPILOT_URL").ok()?;
    match Url::parse(&url) {
        Ok(url) => Some(url),
        Err(err) => {
            tracing::error!(
                ?err,
                value = %url,
                "invalid DRIVER_DELTA_SYNC_AUTOPILOT_URL; delta sync task not started"
            );
            None
        }
    }
}

fn delta_sync_checksum_enabled() -> bool {
    shared::env::flag_enabled(
        std::env::var("DRIVER_DELTA_SYNC_CHECKSUM_ENABLED")
            .ok()
            .as_deref(),
        true,
    )
}

async fn compare_replica_checksum(
    base_url: &Url,
    local_checksum: ReplicaChecksum,
) -> anyhow::Result<()> {
    let url = shared::url::join(base_url, "delta/checksum");
    let response = apply_delta_sync_auth(DELTA_CHECKSUM_CLIENT.get(url))
        .send()
        .await?;
    if response.status() == StatusCode::NO_CONTENT {
        return Ok(());
    }
    let response = response.error_for_status()?;
    let remote = response.json::<DeltaChecksumResponse>().await?;

    if local_checksum.order_uid_hash != remote.order_uid_hash
        || local_checksum.price_hash != remote.price_hash
    {
        metrics::get().delta_replica_diverged_total.inc();
        tracing::warn!(
            local_sequence = local_checksum.sequence,
            remote_sequence = remote.sequence,
            "delta replica checksum mismatch"
        );
    }
    Ok(())
}

fn delta_stream_gone_retry_threshold() -> u64 {
    *DELTA_STREAM_GONE_RETRY_THRESHOLD.get_or_init(|| {
        std::env::var("DRIVER_DELTA_SYNC_GONE_LAG_THRESHOLD")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|value| *value > 0)
            .unwrap_or(16)
    })
}

fn delta_replica_max_staleness() -> Option<Duration> {
    DELTA_REPLICA_MAX_STALENESS
        .get_or_init(|| {
            std::env::var("DRIVER_DELTA_SYNC_MAX_STALENESS_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .map(Duration::from_secs)
        })
        .as_ref()
        .copied()
}

fn delta_replica_resnapshot_interval() -> Option<Duration> {
    DELTA_REPLICA_RESNAPSHOT_INTERVAL
        .get_or_init(|| {
            std::env::var("DRIVER_DELTA_SYNC_RESNAPSHOT_SECS")
                .ok()
                .and_then(|value| value.parse::<u64>().ok())
                .filter(|value| *value > 0)
                .map(Duration::from_secs)
        })
        .as_ref()
        .copied()
}

fn apply_delta_sync_auth(request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    if let Some(api_key) = delta_sync_api_key_from_env() {
        request.header(DELTA_SYNC_API_KEY_HEADER, api_key)
    } else {
        request
    }
}

fn delta_sync_api_key_from_env() -> Option<String> {
    std::env::var("DRIVER_DELTA_SYNC_API_KEY").ok()
}

fn delta_replica() -> Arc<RwLock<Replica>> {
    DELTA_REPLICA
        .read()
        .expect("delta replica lock poisoned")
        .clone()
}

fn parse_sse_block(block: &str) -> (&str, Option<String>) {
    let mut event = "message";
    let mut data_lines = Vec::new();

    for raw_line in block.lines() {
        if raw_line.is_empty() || raw_line.starts_with(':') {
            continue;
        }

        let (field, value) = raw_line
            .split_once(':')
            .map(|(field, value)| (field, value.strip_prefix(' ').unwrap_or(value)))
            .unwrap_or((raw_line, ""));

        match field {
            "event" => event = value,
            "data" => data_lines.push(value),
            _ => {}
        }
    }

    if data_lines.is_empty() {
        (event, None)
    } else {
        (event, Some(data_lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        axum::{
            Json,
            Router,
            extract::Query,
            response::{IntoResponse, Sse, sse},
            routing::get,
        },
        std::sync::Arc,
    };

    #[derive(Clone)]
    struct TestServerState {
        snapshot_json: String,
        observed_after_sequence: Arc<std::sync::Mutex<Option<u64>>>,
    }

    #[derive(serde::Deserialize)]
    struct StreamQuery {
        after_sequence: Option<u64>,
    }

    fn valid_uid(byte: u8) -> String {
        format!("0x{}", format!("{byte:02x}").repeat(56))
    }

    fn valid_order(uid: &str) -> serde_json::Value {
        serde_json::json!({
            "uid": uid,
            "sellToken": "0x0000000000000000000000000000000000000001",
            "buyToken": "0x0000000000000000000000000000000000000002",
            "sellAmount": "1",
            "buyAmount": "1",
            "protocolFees": [],
            "created": 1,
            "validTo": 100,
            "kind": "sell",
            "receiver": null,
            "owner": "0x0000000000000000000000000000000000000003",
            "partiallyFillable": false,
            "executed": "0",
            "preInteractions": [],
            "postInteractions": [],
            "class": "market",
            "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            "signingScheme": "eip712",
            "signature": "0x00",
            "quote": null
        })
    }

    async fn spawn_delta_test_server_with_events(
        snapshot_json: String,
        stream_events: Vec<(String, String)>,
    ) -> (
        Url,
        Arc<std::sync::Mutex<Option<u64>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let observed_after_sequence = Arc::new(std::sync::Mutex::new(None));
        let state = Arc::new(TestServerState {
            snapshot_json,
            observed_after_sequence: Arc::clone(&observed_after_sequence),
        });

        let app = Router::new()
            .route(
                "/delta/snapshot",
                get({
                    let state = Arc::clone(&state);
                    move || {
                        let state = Arc::clone(&state);
                        async move {
                            let value: serde_json::Value =
                                serde_json::from_str(&state.snapshot_json).unwrap();
                            Json(value)
                        }
                    }
                }),
            )
            .route(
                "/delta/stream",
                get({
                    let state = Arc::clone(&state);
                    move |Query(query): Query<StreamQuery>| {
                        let state = Arc::clone(&state);
                        let stream_events = stream_events.clone();
                        async move {
                            *state.observed_after_sequence.lock().unwrap() = query.after_sequence;
                            let events = stream_events.into_iter().map(|(event_name, payload)| {
                                Ok::<_, std::convert::Infallible>(
                                    sse::Event::default().event(event_name).data(payload),
                                )
                            });

                            Sse::new(futures::stream::iter(events)).into_response()
                        }
                    }
                }),
            );

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (
            Url::parse(&format!("http://{addr}")).unwrap(),
            observed_after_sequence,
            handle,
        )
    }

    async fn spawn_delta_test_server(
        snapshot_json: String,
        stream_payload: String,
    ) -> (
        Url,
        Arc<std::sync::Mutex<Option<u64>>>,
        tokio::task::JoinHandle<()>,
    ) {
        spawn_delta_test_server_with_events(
            snapshot_json,
            vec![("delta".to_string(), stream_payload)],
        )
        .await
    }

    #[tokio::test]
    async fn applies_delta_block_to_replica() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 0,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let uid = valid_uid(1);

        let block = format!(
            "event: delta\ndata: \
             {{\"version\":1,\"fromSequence\":0,\"toSequence\":1,\"events\":[{{\"type\":\"\
             orderAdded\",\"order\":{}}}]}}\n\n",
            valid_order(&uid)
        );

        let outcome = handle_sse_block(&block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Continue));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 1);
        assert!(view.orders().contains_key(&uid));
    }

    #[tokio::test]
    async fn resync_event_requests_resnapshot() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        let block = "event: resync_required\ndata: lagged\n\n";

        let outcome = handle_sse_block(&block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Resnapshot));
    }

    #[tokio::test]
    async fn unknown_event_is_ignored() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        let block = "event: keepalive\ndata: ok\n\n";

        let outcome = handle_sse_block(&block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Continue));
    }

    #[tokio::test]
    async fn malformed_delta_payload_returns_error() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        let block = "event: delta\ndata: not-json\n\n";

        let err = handle_sse_block(block, &replica).await.unwrap_err();
        assert!(
            err.to_string().contains("expected ident")
                || err.to_string().contains("expected value")
        );
    }

    #[tokio::test]
    async fn unsupported_delta_version_requests_resnapshot() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 0,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let block = "event: delta\ndata: \
                     {\"version\":2,\"fromSequence\":0,\"toSequence\":1,\"events\":[]}\n\n";

        let outcome = handle_sse_block(&block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Resnapshot));
    }

    #[tokio::test]
    async fn stale_delta_block_is_ignored() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 2,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let uid = valid_uid(1);

        let block = format!(
            "event: delta\ndata: \
             {{\"version\":1,\"fromSequence\":1,\"toSequence\":2,\"events\":[{{\"type\":\"\
             orderRemoved\",\"uid\":\"{uid}\"}}]}}\n\n"
        );

        let outcome = handle_sse_block(&block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Continue));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 2);
    }

    #[tokio::test]
    async fn multiline_delta_data_is_parsed() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 0,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let uid = valid_uid(2);

        let block = format!(
            "event: delta\ndata: {{\"version\":1,\"fromSequence\":0,\ndata: \
             \"toSequence\":1,\"events\":[{{\"type\":\"orderAdded\",\"order\":{}}}]}}\n\n",
            valid_order(&uid)
        );

        let outcome = handle_sse_block(&block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Continue));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 1);
        assert!(view.orders().contains_key(&uid));
    }

    #[tokio::test]
    async fn comments_and_unknown_fields_are_ignored() {
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 0,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let block = r#":keepalive
id: 9
retry: 5000
event: delta
data: {"version":1,"fromSequence":0,"toSequence":1,"events":[]}

"#;

        let outcome = handle_sse_block(block, &replica).await.unwrap();
        assert!(matches!(outcome, BlockControl::Continue));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 1);
    }

    #[test]
    fn replica_preprocessing_flag_parsing() {
        assert!(!shared::env::flag_enabled(None, false));
        assert!(!shared::env::flag_enabled(Some("false"), false));
        assert!(!shared::env::flag_enabled(Some("0"), false));
        assert!(shared::env::flag_enabled(Some("true"), false));
        assert!(shared::env::flag_enabled(Some("on"), false));
    }

    #[tokio::test]
    async fn fetch_snapshot_reads_bootstrap_state_from_http_endpoint() {
        let uid = valid_uid(1);
        let snapshot = serde_json::json!({
            "version": 1,
            "auctionId": 0,
            "sequence": 7,
            "auction": {
                "orders": [valid_order(&uid)],
                "prices": {"0x0101010101010101010101010101010101010101": "123"}
            }
        });
        let (base_url, _after_sequence, server) =
            spawn_delta_test_server(snapshot.to_string(), "{}".to_string()).await;
        let client = reqwest::Client::new();

        let fetched = fetch_snapshot(&client, &base_url).await.unwrap();

        assert_eq!(fetched.version, 1);
        assert_eq!(fetched.sequence, 7);
        assert_eq!(fetched.auction.orders.len(), 1);
        assert_eq!(fetched.auction.prices.len(), 1);

        server.abort();
    }

    #[tokio::test]
    async fn follow_stream_applies_delta_and_uses_after_sequence_query() {
        let uid = valid_uid(1);
        let snapshot = serde_json::json!({
            "version": 1,
            "auctionId": 0,
            "sequence": 3,
            "auction": {
                "orders": [valid_order(&uid)],
                "prices": {}
            }
        });
        let stream_delta = serde_json::json!({
            "version": 1,
            "fromSequence": 3,
            "toSequence": 4,
            "events": [{"type": "orderUpdated", "order": valid_order(&uid)}]
        });
        let (base_url, observed_after_sequence, server) =
            spawn_delta_test_server(snapshot.to_string(), stream_delta.to_string()).await;

        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 3,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![valid_order(&uid)],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let client = reqwest::Client::new();
        let err = follow_stream(&client, &base_url, &replica, Instant::now())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("delta stream closed"));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 4);
        assert!(view.orders().contains_key(&uid));
        assert_eq!(*observed_after_sequence.lock().unwrap(), Some(3));

        server.abort();
    }

    #[tokio::test]
    async fn follow_stream_requests_resnapshot_on_out_of_order_delta() {
        let uid = valid_uid(1);
        let snapshot = serde_json::json!({
            "version": 1,
            "auctionId": 0,
            "sequence": 10,
            "auction": {
                "orders": [valid_order(&uid)],
                "prices": {}
            }
        });
        let out_of_order_delta = serde_json::json!({
            "version": 1,
            "fromSequence": 9,
            "toSequence": 11,
            "events": []
        });
        let (base_url, _after_sequence, server) =
            spawn_delta_test_server(snapshot.to_string(), out_of_order_delta.to_string()).await;

        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 10,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![valid_order(&uid)],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let client = reqwest::Client::new();
        let control = follow_stream(&client, &base_url, &replica, Instant::now())
            .await
            .unwrap();
        assert!(matches!(control, StreamControl::Resnapshot));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 10);
        assert!(view.orders().contains_key(&uid));

        server.abort();
    }

    #[tokio::test]
    async fn bootstrap_then_streamed_deltas_converge_to_expected_state() {
        let token_a = alloy::primitives::Address::repeat_byte(0xAA);
        let token_b = alloy::primitives::Address::repeat_byte(0xBB);
        let token_c = alloy::primitives::Address::repeat_byte(0xCC);
        let uid_one = valid_uid(1);
        let uid_two = valid_uid(2);
        let uid_three = valid_uid(3);

        let snapshot = serde_json::json!({
            "version": 1,
            "auctionId": 0,
            "sequence": 2,
            "auction": {
                "orders": [
                    valid_order(&uid_one),
                    valid_order(&uid_two)
                ],
                "prices": {
                    token_a.to_string(): "100",
                    token_b.to_string(): "200"
                }
            }
        });
        let delta_one = serde_json::json!({
            "version": 1,
            "fromSequence": 2,
            "toSequence": 3,
            "events": [
                {"type": "orderUpdated", "order": valid_order(&uid_one)},
                {"type": "orderRemoved", "uid": uid_two},
                {"type": "orderAdded", "order": valid_order(&uid_three)},
                {"type": "priceChanged", "token": token_b, "price": null},
                {"type": "priceChanged", "token": token_c, "price": "300"}
            ]
        });
        let delta_two = serde_json::json!({
            "version": 1,
            "fromSequence": 3,
            "toSequence": 4,
            "events": [
                {"type": "orderUpdated", "order": valid_order(&uid_three)}
            ]
        });

        let (base_url, observed_after_sequence, server) = spawn_delta_test_server_with_events(
            snapshot.to_string(),
            vec![
                ("delta".to_string(), delta_one.to_string()),
                ("delta".to_string(), delta_two.to_string()),
            ],
        )
        .await;

        let client = reqwest::Client::new();
        let snapshot_payload = fetch_snapshot(&client, &base_url).await.unwrap();
        let replica = std::sync::Arc::new(RwLock::new(Replica::default()));
        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(snapshot_payload).unwrap();
        }

        let err = follow_stream(&client, &base_url, &replica, Instant::now())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("delta stream closed"));

        let view = replica.read().await;
        assert_eq!(view.sequence(), 4);
        assert_eq!(*observed_after_sequence.lock().unwrap(), Some(2));

        assert_eq!(view.orders().len(), 2);
        assert!(view.orders().contains_key(&uid_one));
        assert!(view.orders().contains_key(&uid_three));
        assert!(!view.orders().contains_key(&uid_two));

        assert_eq!(view.prices().len(), 2);
        assert_eq!(view.prices().get(&token_a).unwrap(), "100");
        assert_eq!(view.prices().get(&token_c).unwrap(), "300");
        assert!(!view.prices().contains_key(&token_b));

        server.abort();
    }

    #[tokio::test]
    async fn replica_health_reports_state_and_age() {
        reset_delta_replica_for_tests();
        let replica = delta_replica();

        {
            let mut lock = replica.write().await;
            lock.apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 1,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![valid_order(&valid_uid(1))],
                    prices: HashMap::new(),
                },
            })
            .unwrap();
        }

        let health = replica_health()
            .await
            .expect("replica health should be available");
        assert!(matches!(health.state, ReplicaState::Ready));
        assert_eq!(health.sequence, 1);
        assert_eq!(health.order_count, 1);
        assert!(health.last_update.is_some());
        assert!(health.last_update_age_seconds.is_some());
    }

    #[test]
    fn snapshot_requires_ready_state() {
        reset_delta_replica_for_tests();
        let mut replica = Replica::default();
        replica
            .apply_snapshot(Snapshot {
                version: 1,
                auction_id: 0,
                sequence: 1,
                auction: crate::domain::competition::delta_replica::RawAuctionData {
                    orders: vec![valid_order(&valid_uid(1))],
                    prices: HashMap::new(),
                },
            })
            .unwrap();

        replica.set_state(ReplicaState::Syncing);
        assert!(snapshot_from_replica(&replica).is_none());

        replica.set_state(ReplicaState::Ready);
        assert!(snapshot_from_replica(&replica).is_some());
    }
}
