//! Background websocket stream of eth_call-style state overrides for
//! live-quote venues whose current price lives in maker memory rather than
//! committed on-chain state (e.g. Titan propAMMs). Applied on top of latest
//! state during settlement gas estimation and trade verification so pAMM
//! routes simulate against their current in-memory state instead of stale
//! previous-block state.

use {
    alloy_primitives::Address,
    alloy_rpc_types::state::StateOverride,
    configs::simulator::StateOverrideStream as Config,
    ethrpc::block_stream::CurrentBlockWatcher,
    futures::{SinkExt, StreamExt},
    prometheus::{IntCounter, IntCounterVec, IntGauge},
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::watch,
    tracing::{debug, warn},
};

/// State overrides delivered some point in time.
#[derive(Clone)]
struct Snapshot {
    overrides: StateOverride,
    block_number: u64,
    received_at: Option<Instant>,
}

#[derive(Clone)]
pub struct SimulationOverrides(Arc<Inner>);

struct Inner {
    snapshots: watch::Receiver<Snapshot>,
    current_block: CurrentBlockWatcher,
    max_age: Duration,
}

impl std::fmt::Debug for SimulationOverrides {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimulationOverrides")
            .field("max_age", &self.0.max_age)
            .finish()
    }
}

impl SimulationOverrides {
    /// Returns the live state overrides, or `None` (callers omit the RPC
    /// override param entirely) when the stream is stale or unconfigured.
    pub fn current(&self) -> Option<StateOverride> {
        let snapshot = self.0.snapshots.borrow();
        let received_at = snapshot.received_at?;
        if received_at.elapsed() > self.0.max_age {
            Metrics::get().record_override_result(OverrideResult::Stale);
            return None;
        }
        let current_block_number = self.0.current_block.borrow().number;
        if snapshot.block_number != current_block_number {
            Metrics::get().record_override_result(OverrideResult::Stale);
            return None;
        }
        if snapshot.overrides.is_empty() {
            Metrics::get().record_override_result(OverrideResult::Stale);
            return None;
        }
        Metrics::get().record_override_result(OverrideResult::Fresh);
        Some(snapshot.overrides.clone())
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    block_number: Option<u64>,
    // Venue keys are addresses flattened alongside the metadata fields above;
    // unknown non-address keys (e.g. future schema additions) are skipped by
    // the address parse inside the deserializer.
    #[serde(flatten, deserialize_with = "deserialize_venue_overrides")]
    venues: BTreeMap<Address, VenueUpdate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct VenueUpdate {
    state_override: StateOverride,
}

fn deserialize_venue_overrides<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<Address, VenueUpdate>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Vis;

    impl<'de> serde::de::Visitor<'de> for Vis {
        type Value = BTreeMap<Address, VenueUpdate>;

        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a map of venue address overrides")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::MapAccess<'de>,
        {
            let mut out = BTreeMap::new();
            while let Some(key) = map.next_key::<&str>()? {
                let Ok(address) = key.parse::<Address>() else {
                    map.next_value::<serde::de::IgnoredAny>()?;
                    continue;
                };
                out.insert(address, map.next_value::<VenueUpdate>()?);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(Vis)
}

/// Spawns a new background task that streams state override updates
/// into the return [`SimulationOverrides`] instance.
pub fn spawn(cfg: &Config, current_block: CurrentBlockWatcher) -> SimulationOverrides {
    let (sender, receiver) = watch::channel(Snapshot {
        overrides: StateOverride::default(),
        block_number: 0,
        received_at: None,
    });

    let ws_url = cfg.ws_url.clone();
    tokio::spawn(async move {
        run_stream(ws_url, sender).await;
    });

    SimulationOverrides(Arc::new(Inner {
        snapshots: receiver,
        current_block,
        max_age: cfg.max_age,
    }))
}

async fn run_stream(ws_url: url::Url, sender: watch::Sender<Snapshot>) {
    let mut backoff = Duration::from_millis(250);
    let mut overrides = StateOverride::default();
    let mut last_block_number = 0u64;

    loop {
        match tokio_tungstenite::connect_async(ws_url.as_str()).await {
            Ok((ws_stream, _)) => {
                backoff = Duration::from_millis(250);
                let (mut write, mut read) = ws_stream.split();
                debug!(url = %ws_url, "state-override stream connected");

                while let Some(message) = read.next().await {
                    let message = match message {
                        Ok(message) => message,
                        Err(err) => {
                            warn!(?err, "state-override stream read error");
                            break;
                        }
                    };

                    if !message.is_text() && !message.is_binary() {
                        continue;
                    }

                    Metrics::get().frames_received.inc();
                    match serde_json::from_slice::<Frame>(&message.into_data()) {
                        Ok(frame) => {
                            if let Some(block_number) = frame.block_number {
                                last_block_number = block_number;
                            }
                            apply_frame(&mut overrides, frame);
                            publish(&overrides, last_block_number, &sender);
                        }
                        Err(err) => {
                            Metrics::get().parse_failures.inc();
                            debug!(?err, "state-override stream frame parse error");
                        }
                    }
                }

                let _ = write.close().await;
            }
            Err(err) => {
                warn!(?err, url = %ws_url, "state-override stream connect failed");
            }
        }

        Metrics::get().reconnects.inc();
        debug!(?backoff, "state-override stream reconnecting");
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(15));
    }
}

// Each frame's stateOverride is a full per-venue snapshot, not a delta, so
// accounts are inserted directly (latest frame wins per account).
fn apply_frame(overrides: &mut StateOverride, frame: Frame) {
    for update in frame.venues.into_values() {
        for (account, account_override) in update.state_override {
            overrides.insert(account, account_override);
        }
    }
}

fn publish(overrides: &StateOverride, block_number: u64, sender: &watch::Sender<Snapshot>) {
    Metrics::get().venue_count.set(overrides.len() as i64);
    let snapshot = Snapshot {
        overrides: overrides.clone(),
        block_number,
        received_at: Some(Instant::now()),
    };
    if let Err(err) = sender.send(snapshot) {
        tracing::warn!(?err, "receiver of state override updates dropped");
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Total state-override frames received from the websocket.
    frames_received: IntCounter,
    /// Frames that failed to parse.
    parse_failures: IntCounter,
    /// Reconnect attempts.
    reconnects: IntCounter,
    /// Cross-venue override conflicts (no longer incremented; latest frame
    /// wins per account via insert, but kept for metric stability).
    merge_conflicts: IntCounter,
    /// Accounts in the merged state-override snapshot.
    venue_count: IntGauge,
    /// Gas simulations by whether overrides were applied.
    #[metric(labels("result"))]
    simulations_with_overrides: IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Metrics {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    fn record_override_result(&self, result: OverrideResult) {
        self.simulations_with_overrides
            .with_label_values(&[result.as_str()])
            .inc();
    }
}

enum OverrideResult {
    Fresh,
    Stale,
}

impl OverrideResult {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Stale => "stale",
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{B256, U256, address},
        alloy_rpc_types::state::AccountOverride,
        ethrpc::block_stream::BlockInfo,
        futures::StreamExt,
        std::time::Duration,
        tokio::time::timeout,
        tokio_tungstenite::tungstenite::Message,
    };

    fn block(number: u64, timestamp: u64) -> (watch::Sender<BlockInfo>, CurrentBlockWatcher) {
        let (tx, rx) = watch::channel(BlockInfo {
            number,
            timestamp,
            ..Default::default()
        });
        (tx, rx)
    }

    fn frame_with(
        venue: Address,
        account: Address,
        balance: Option<U256>,
        slot: Option<B256>,
    ) -> Frame {
        let mut account_override = AccountOverride::default();
        if let Some(balance) = balance {
            account_override.balance = Some(balance);
        }
        if let Some(slot) = slot {
            let mut diff = alloy_primitives::map::B256Map::default();
            diff.insert(slot, B256::ZERO);
            account_override.state_diff = Some(diff);
        }
        let mut state_override = StateOverride::default();
        state_override.insert(account, account_override);
        let mut venues = BTreeMap::new();
        venues.insert(venue, VenueUpdate { state_override });
        Frame {
            block_number: None,
            venues,
        }
    }

    #[test]
    fn frame_parses_verbatim_titan_sample() {
        let sample = r#"{
            "slot": 14285824,
            "blockNumber": 25051224,
            "timestamp": 1778253913749564761,
            "future-metadata": "ignored",
            "0x1111111111111111111111111111111111111111": {
                "stateOverride": {
                    "0x2222222222222222222222222222222222222222": {
                        "balance": "0x0",
                        "nonce": "0x1",
                        "stateDiff": { "0x0000000000000000000000000000000000000000000000000000000000000001": "0x0000000000000000000000000000000000000000000000000000000000000000" }
                    }
                }
            },
            "not-an-address": { "stateOverride": {} }
        }"#;
        let frame: Frame = serde_json::from_str(sample).unwrap();
        assert_eq!(frame.block_number, Some(25051224));
        // Non-address top-level keys ("future-metadata", "not-an-address")
        // are skipped by the venue deserializer.
        assert_eq!(frame.venues.len(), 1);

        let venue = address!("1111111111111111111111111111111111111111");
        let account = address!("2222222222222222222222222222222222222222");
        let update = frame.venues.get(&venue).unwrap();
        let override_entry = update.state_override.get(&account).unwrap();
        assert_eq!(override_entry.nonce, Some(1));
        assert!(override_entry.state_diff.is_some());
    }

    #[test]
    fn apply_frame_replaces_per_venue_state() {
        let venue = address!("1111111111111111111111111111111111111111");
        let account = address!("2222222222222222222222222222222222222222");

        let mut overrides = StateOverride::default();
        apply_frame(
            &mut overrides,
            frame_with(venue, account, None, Some(B256::ZERO)),
        );
        assert!(overrides.get(&account).unwrap().state_diff.is_some());
        assert!(overrides.get(&account).unwrap().balance.is_none());

        apply_frame(
            &mut overrides,
            frame_with(venue, account, Some(U256::ZERO), None),
        );
        assert_eq!(overrides.get(&account).unwrap().balance, Some(U256::ZERO));
        assert!(overrides.get(&account).unwrap().state_diff.is_none());
    }

    #[test]
    fn apply_frame_merges_disjoint_accounts() {
        let venue_a = address!("1111111111111111111111111111111111111111");
        let venue_b = address!("3333333333333333333333333333333333333333");
        let account_a = address!("2222222222222222222222222222222222222222");
        let account_b = address!("4444444444444444444444444444444444444444");

        let mut overrides = StateOverride::default();
        apply_frame(
            &mut overrides,
            frame_with(venue_a, account_a, None, Some(B256::ZERO)),
        );
        apply_frame(
            &mut overrides,
            frame_with(venue_b, account_b, None, Some(B256::ZERO)),
        );
        assert!(overrides.contains_key(&account_a));
        assert!(overrides.contains_key(&account_b));
    }

    #[test]
    fn apply_frame_conflict_latest_wins() {
        let venue_a = address!("1111111111111111111111111111111111111111");
        let venue_b = address!("3333333333333333333333333333333333333333");
        let shared = address!("2222222222222222222222222222222222222222");

        let mut overrides = StateOverride::default();
        apply_frame(
            &mut overrides,
            frame_with(venue_a, shared, Some(U256::from(1)), None),
        );
        apply_frame(
            &mut overrides,
            frame_with(venue_b, shared, Some(U256::from(2)), None),
        );
        assert_eq!(overrides.get(&shared).unwrap().balance, Some(U256::from(2)));
    }

    fn handle(
        receiver: watch::Receiver<Snapshot>,
        block_rx: CurrentBlockWatcher,
        max_age: Duration,
    ) -> SimulationOverrides {
        SimulationOverrides(Arc::new(Inner {
            snapshots: receiver,
            current_block: block_rx,
            max_age,
        }))
    }

    fn non_empty_snapshot(block_number: u64, received_at: Instant) -> Snapshot {
        let mut overrides = StateOverride::default();
        overrides.insert(
            address!("1111111111111111111111111111111111111111"),
            AccountOverride::default(),
        );
        Snapshot {
            overrides,
            block_number,
            received_at: Some(received_at),
        }
    }

    #[test]
    fn staleness_gates_return_none() {
        // Stale by age.
        let (_sender, receiver) = watch::channel(non_empty_snapshot(
            100,
            Instant::now() - Duration::from_millis(100),
        ));
        let (_, block_rx) = block(100, 1000);
        assert!(
            handle(receiver, block_rx, Duration::from_millis(50))
                .current()
                .is_none()
        );

        // Stale by block mismatch: snapshot block 100, head block 105.
        let (_sender, receiver) = watch::channel(non_empty_snapshot(100, Instant::now()));
        let (_, block_rx) = block(105, 1000);
        assert!(
            handle(receiver, block_rx, Duration::from_secs(30))
                .current()
                .is_none()
        );

        // Stale because the merged overrides are empty.
        let (sender, receiver) = watch::channel(Snapshot {
            overrides: StateOverride::default(),
            block_number: 100,
            received_at: Some(Instant::now()),
        });
        let (_, block_rx) = block(100, 1000);
        let h = handle(receiver, block_rx, Duration::from_secs(30));
        sender
            .send(Snapshot {
                overrides: StateOverride::default(),
                block_number: 100,
                received_at: Some(Instant::now()),
            })
            .unwrap();
        assert!(h.current().is_none());
    }

    #[tokio::test]
    async fn reconnect_and_accumulate_against_in_process_server() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_url: url::Url = format!("ws://{addr}").parse().unwrap();
        let account = address!("2222222222222222222222222222222222222222");
        let first = r#"{"blockNumber":10,"0x1111111111111111111111111111111111111111":{"stateOverride":{"0x2222222222222222222222222222222222222222":{"balance":"0x1"}}}}"#;
        let second = r#"{"blockNumber":11,"0x1111111111111111111111111111111111111111":{"stateOverride":{"0x2222222222222222222222222222222222222222":{"balance":"0x2"}}}}"#;

        let server_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let ws = tokio_tungstenite::accept_async(stream).await.unwrap();
            let (mut write, _read) = ws.split();
            write.send(Message::Text(first.into())).await.unwrap();
            write.send(Message::Text(second.into())).await.unwrap();
            write.close().await.unwrap();
        });

        let (_, block_rx) = block(11, 1000);
        let cfg = Config {
            ws_url: server_url,
            max_age: Duration::from_secs(30),
        };
        let handle = spawn(&cfg, block_rx);

        let _ = server_handle.await;

        let got = timeout(Duration::from_secs(2), async {
            loop {
                if let Some(overrides) = handle.current()
                    && let Some(account_override) = overrides.get(&account)
                    && account_override.balance == Some(U256::from(2))
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        assert!(got.is_ok(), "did not observe merged overrides in time");
    }

    // Real frames captured from wss://eu.rpc.titanbuilder.xyz/ws/pamm_quote_stream.
    const FERMI_FRAME: &str = r#"{"slot":14711587,"blockNumber":25475333,"timestamp":1783363067584411872,"0xb1076fe3ab5e28005c7c323bac5ac06a680d452e":{"stateOverride":{"0xa048e0c08b7acb48363711800ac9d49de8e58d13":{"balance":"0x10848dc44f1e140","nonce":"0x42ee"},"0x14e870f0a7c764ca71289952006d6bf130058927":{"balance":"0x10aaf167cb7dbea","nonce":"0x23da"},"0x69939a6c590c9cd0bf8efbe9b3df2cdac4a4906b":{"balance":"0x88ecb0471d376e","nonce":"0x2fad"},"0xfc42be9494f1af6b03adad71811c62ada2d6f3c3":{"balance":"0x10e7e3c0b4f8ba0","nonce":"0x1d99"},"0xda7afeed01fe625cf15d187a19f94b45f00b8c5f":{"balance":"0x0","nonce":"0x1","stateDiff":{"0x6d3af688dd77e4167e6ad8613dea4a162f5e340043b2c026e3d2b5b40d12c92d":"0x6a4bf5fb010000000000000000000000000000000000000000000029cdbee960","0x9a965d2bccf7f891d58fe85acac20d9de58c11ac1d222dfff7973a09ad71143a":"0x6a4bf5fb010000000000000000000000000000000000000000000029c8581698","0xe25ff9533ce41163d3738b63c7d954cd7449a0ba0dd0dac8db25ae29536b4961":"0x6a4bf5fb010000000000000000000000000000000000000000000029cdbee960","0x939ee2e42000f154d3be2302ab4d3cb916e4b2852ef6a0caa2fd76c417120248":"0x6a4bf5fb010000000000000000000000000000000000000000000029c8581698"}}}}}"#;
    const OTHER_FRAME: &str = r#"{"slot":14711587,"blockNumber":25475333,"timestamp":1783363067506613546,"0x28d9ccedf1b7ac9b3f090f4f0292837de87c1d39":{"stateOverride":{"0x28d9ccedf1b7ac9b3f090f4f0292837de87c1d39":{"balance":"0x0","nonce":"0x1","stateDiff":{"0xe3ffa73f3a3b56e693c2ed775464cb3fbe78307b000000000000000000000000":"0x5d393a1348485d39e6f3484885cda948444485cd9644363600019f38b8de3300"}},"0xe3ffa73f3a3b56e693c2ed775464cb3fbe78307b":{"balance":"0x12e8705b8c388ab1","nonce":"0xdda1"}}}}"#;

    #[test]
    fn parses_real_titan_frames() {
        let fermi: Frame = serde_json::from_str(FERMI_FRAME).unwrap();
        assert_eq!(fermi.block_number, Some(25475333));
        assert_eq!(fermi.venues.len(), 1);

        let mut overrides = StateOverride::default();
        apply_frame(&mut overrides, fermi);
        assert_eq!(overrides.len(), 5);

        let venue_account = address!("da7afeed01fe625cf15d187a19f94b45f00b8c5f");
        let entry = overrides.get(&venue_account).unwrap();
        assert_eq!(entry.balance, Some(U256::ZERO));
        assert_eq!(entry.nonce, Some(1));
        let diff = entry.state_diff.as_ref().unwrap();
        assert_eq!(diff.len(), 4);
        let slot: alloy_primitives::B256 =
            "0x6d3af688dd77e4167e6ad8613dea4a162f5e340043b2c026e3d2b5b40d12c92d"
                .parse()
                .unwrap();
        assert!(diff.contains_key(&slot));
    }

    #[test]
    fn accumulates_real_frames_across_venues() {
        let mut overrides = StateOverride::default();
        apply_frame(&mut overrides, serde_json::from_str(FERMI_FRAME).unwrap());
        apply_frame(&mut overrides, serde_json::from_str(OTHER_FRAME).unwrap());

        assert_eq!(overrides.len(), 7);
        assert!(overrides.contains_key(&address!("da7afeed01fe625cf15d187a19f94b45f00b8c5f")));
        assert!(overrides.contains_key(&address!("e3ffa73f3a3b56e693c2ed775464cb3fbe78307b")));
    }

    #[test]
    fn current_yields_state_overrides_for_real_data() {
        let (block_tx, block_rx) = block(25475333, 1783363000);
        let (_sender, receiver) = watch::channel(Snapshot {
            overrides: {
                let mut m = StateOverride::default();
                apply_frame(&mut m, serde_json::from_str(FERMI_FRAME).unwrap());
                m
            },
            block_number: 25475333,
            received_at: Some(Instant::now()),
        });
        let handle = SimulationOverrides(Arc::new(Inner {
            snapshots: receiver,
            current_block: block_rx,
            max_age: Duration::from_secs(30),
        }));

        let overrides = handle.current().unwrap();
        assert_eq!(overrides.len(), 5);

        block_tx
            .send(BlockInfo {
                number: 25475335,
                timestamp: 1783363000,
                ..Default::default()
            })
            .unwrap();
        assert!(handle.current().is_none());
    }
}
