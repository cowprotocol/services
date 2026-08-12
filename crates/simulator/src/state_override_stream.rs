//! Background websocket stream of eth_call-style state overrides for
//! live-quote venues whose current price lives in maker memory rather than
//! committed on-chain state (e.g. Titan propAMMs). Applied on top of latest
//! state during settlement gas estimation and trade verification so pAMM
//! routes simulate against their current in-memory state instead of stale
//! previous-block state.
//!
//! Terminology used below, none of which the venue's own docs name:
//! * *frame* — one websocket message: one venue's overrides for the block the
//!   builder is about to build.
//! * *lane* — one storage slot of the shared registry, holding one venue's live
//!   quote for one token pair and direction. Venues quote a lane at most once
//!   per block and leave the ones they aren't quoting alone.
//! * *stamp* — the leading four bytes of a lane's word, holding the timestamp
//!   of the block the quote is meant for. The venue reverts `StaleUpdate()`
//!   unless it equals the timestamp of the block the call runs in; the
//!   remaining 28 bytes are the maker's price.

use {
    alloy_primitives::{Address, B256, map::B256Map},
    alloy_rpc_types::state::{AccountOverride, StateOverride},
    configs::simulator::StateOverrideStream as Config,
    futures::{SinkExt, StreamExt},
    prometheus::{IntCounter, IntCounterVec, IntGauge},
    serde::Deserialize,
    std::{
        collections::BTreeMap,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::watch,
};

/// Number of leading bytes of a storage word holding the venue's freshness
/// stamp. The remaining bytes are the maker's price and must survive
/// restamping untouched.
const STAMP_LEN: usize = 4;

/// Beacon chain genesis and slot spacing, which turn the slot a frame names
/// into the timestamp its freshly quoted lanes are stamped with.
///
/// These are mainnet's, which is the only chain the venues stream for. On any
/// other chain the derived stamp matches no word, so no lane is recognised as
/// freshly quoted and nothing is restamped.
const BEACON_GENESIS: u64 = 1_606_824_023;
const SLOT_DURATION: u64 = 12;

/// State overrides delivered some point in time.
#[derive(Clone)]
struct Snapshot {
    overrides: StateOverride,
    /// Block the frames describe. This is the block the builder is about to
    /// build (chain head + 1), not the block a simulation runs against.
    block_number: u64,
    /// Stamp of the newest slot any venue quoted for. Only words carrying it
    /// belong to a lane a maker is quoting for `block_number`.
    stamp: Option<u32>,
    received_at: Option<Instant>,
}

#[derive(Clone)]
pub struct SimulationOverrides(Arc<Inner>);

struct Inner {
    snapshots: watch::Receiver<Snapshot>,
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
    /// Returns the live state overrides adjusted for a simulation running at
    /// `block` with `timestamp`, or `None` (callers omit the RPC override
    /// param entirely) when the stream can't serve that context.
    ///
    /// Frames describe the block the builder is about to build, so a snapshot
    /// ahead of the simulated block is the normal case: its freshness stamp is
    /// rewritten to `timestamp` so the venue accepts the quote in the context
    /// it is actually evaluated in. A snapshot *behind* the simulated block is
    /// withheld instead, because the maker has already moved on from the price
    /// it carries.
    pub fn overrides_for(&self, block: u64, timestamp: u64) -> Option<StateOverride> {
        let metrics = Metrics::get();
        // Holding this borrow blocks the stream task from publishing, so it is
        // released before the copy is restamped.
        let (overrides, stamp) = {
            let snapshot = self.0.snapshots.borrow();
            let Some(received_at) = snapshot.received_at else {
                metrics.record_override_result(OverrideResult::Empty);
                return None;
            };
            if received_at.elapsed() > self.0.max_age {
                metrics.record_override_result(OverrideResult::TooOld);
                return None;
            }
            if snapshot.block_number < block {
                metrics.record_override_result(OverrideResult::WrongBlock);
                return None;
            }
            if snapshot.overrides.is_empty() {
                metrics.record_override_result(OverrideResult::Empty);
                return None;
            }
            (snapshot.overrides.clone(), snapshot.stamp)
        };
        metrics.record_override_result(OverrideResult::Fresh);
        let overrides = restamp(overrides, stamp, timestamp);
        Some(overrides)
    }
}

/// Moves the lanes quoted for `stamp` into the simulated block by rewriting
/// their stamp to `timestamp`.
///
/// Only words carrying `stamp` are rewritten. A lane the maker did not requote
/// keeps its older stamp and stays dead, so the venue rejects it exactly as it
/// would on chain; rewriting it too would forge liveness for a price nobody is
/// quoting. Only the stamp bytes are touched, never the price bytes next to
/// them.
fn restamp(mut overrides: StateOverride, stamp: Option<u32>, timestamp: u64) -> StateOverride {
    let Some(stamp) = stamp else {
        return overrides;
    };
    let stamp = stamp.to_be_bytes();
    let timestamp = (timestamp as u32).to_be_bytes();
    for account in overrides.values_mut() {
        let words = [account.state.as_mut(), account.state_diff.as_mut()];
        for word in words.into_iter().flatten().flat_map(B256Map::values_mut) {
            if word[..STAMP_LEN] == stamp {
                word[..STAMP_LEN].copy_from_slice(&timestamp);
            }
        }
    }
    overrides
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Frame {
    slot: Option<u64>,
    block_number: Option<u64>,
    // Venue keys are addresses flattened alongside the metadata fields above;
    // unknown non-address keys (e.g. future schema additions) are skipped by
    // the address parse inside the deserializer.
    #[serde(flatten, deserialize_with = "deserialize_venue_overrides")]
    venues: BTreeMap<Address, VenueUpdate>,
}

impl Frame {
    /// Timestamp of the block this frame quotes for, which is the stamp its
    /// freshly quoted lanes carry.
    fn quoted_at(&self) -> Option<u32> {
        let timestamp = SLOT_DURATION
            .checked_mul(self.slot?)?
            .checked_add(BEACON_GENESIS)?;
        u32::try_from(timestamp).ok()
    }
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
pub fn spawn(cfg: &Config) -> SimulationOverrides {
    let (sender, receiver) = watch::channel(Snapshot {
        overrides: StateOverride::default(),
        block_number: 0,
        stamp: None,
        received_at: None,
    });

    let ws_url = cfg.ws_url.clone();
    tokio::spawn(async move {
        run_stream(ws_url, sender).await;
    });

    SimulationOverrides(Arc::new(Inner {
        snapshots: receiver,
        max_age: cfg.max_age,
    }))
}

async fn run_stream(ws_url: url::Url, sender: watch::Sender<Snapshot>) {
    let mut backoff = Duration::from_millis(250);
    let mut venues = Venues::default();
    let mut last_block_number = 0u64;

    loop {
        match tokio_tungstenite::connect_async(ws_url.as_str()).await {
            Ok((ws_stream, _)) => {
                backoff = Duration::from_millis(250);
                let (mut write, mut read) = ws_stream.split();
                tracing::debug!(url = %ws_url, "state-override stream connected");

                while let Some(message) = read.next().await {
                    let message = match message {
                        Ok(message) => message,
                        Err(err) => {
                            tracing::warn!(?err, "state-override stream read error");
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
                            venues.update(frame);
                            publish(&venues, last_block_number, &sender);
                        }
                        Err(err) => {
                            Metrics::get().parse_failures.inc();
                            tracing::debug!(?err, "state-override stream frame parse error");
                        }
                    }
                }

                let _ = write.close().await;
            }
            Err(err) => {
                tracing::warn!(?err, url = %ws_url, "state-override stream connect failed");
            }
        }

        Metrics::get().reconnects.inc();
        tracing::debug!(?backoff, "state-override stream reconnecting");
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(Duration::from_secs(15));
    }
}

struct Quotes {
    overrides: StateOverride,
    /// Block timestamp this venue's freshly quoted lanes are stamped with,
    /// when its newest frame quoted any.
    stamp: Option<u32>,
}

/// The newest frame of every venue seen so far.
#[derive(Default)]
struct Venues(BTreeMap<Address, Quotes>);

impl Venues {
    /// Replaces each venue's quotes with the ones the frame carries. A frame
    /// holds the venue's whole set of lanes, including the ones it did not
    /// requote this block, so the previous frame is dropped rather than merged:
    /// accumulating would keep lanes of a venue that has gone away alive
    /// forever.
    fn update(&mut self, frame: Frame) {
        let quoted_at = frame.quoted_at();
        for (venue, update) in frame.venues {
            let overrides = update.state_override;
            let stamp = quoted_at.filter(|stamp| {
                let stamp = stamp.to_be_bytes();
                words(&overrides).any(|word| word[..STAMP_LEN] == stamp)
            });
            self.0.insert(venue, Quotes { overrides, stamp });
        }
    }

    /// Folds every venue's newest frame into one override set, along with the
    /// newest stamp any of them quoted.
    ///
    /// Every venue keeps its lanes in the same shared registry account and a
    /// frame only ever carries its own, so storage is merged word by word:
    /// inserting the account wholesale would drop the other venues' lanes.
    fn fold(&self) -> (StateOverride, Option<u32>) {
        let mut overrides = StateOverride::default();
        let mut stamp = None;
        for quotes in self.0.values() {
            stamp = stamp.max(quotes.stamp);
            for (account, account_override) in &quotes.overrides {
                merge_account(
                    overrides.entry(*account).or_default(),
                    account_override.clone(),
                );
            }
        }
        (overrides, stamp)
    }
}

fn words(overrides: &StateOverride) -> impl Iterator<Item = &B256> {
    overrides.values().flat_map(|account| {
        [account.state.as_ref(), account.state_diff.as_ref()]
            .into_iter()
            .flatten()
            .flat_map(B256Map::values)
    })
}

/// Folds `update` into `target`. A frame only ever carries the fields its own
/// venue set, so a field it leaves out means "unchanged", not "cleared" — the
/// same account is described by several venues' frames.
fn merge_account(target: &mut AccountOverride, update: AccountOverride) {
    let AccountOverride {
        balance,
        nonce,
        code,
        state,
        state_diff,
        move_precompile_to,
    } = update;
    target.balance = balance.or(target.balance);
    target.nonce = nonce.or(target.nonce);
    target.code = code.or_else(|| target.code.take());
    target.move_precompile_to = move_precompile_to.or(target.move_precompile_to);
    merge_words(&mut target.state, state);
    merge_words(&mut target.state_diff, state_diff);
}

fn merge_words(target: &mut Option<B256Map<B256>>, update: Option<B256Map<B256>>) {
    let Some(update) = update else {
        return;
    };
    target.get_or_insert_default().extend(update);
}

fn publish(venues: &Venues, block_number: u64, sender: &watch::Sender<Snapshot>) {
    let (overrides, stamp) = venues.fold();
    Metrics::get().venue_count.set(overrides.len() as i64);
    let snapshot = Snapshot {
        overrides,
        block_number,
        stamp,
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
    /// Cross-venue override conflicts (no longer incremented; frames are
    /// merged word by word, but kept for metric stability).
    merge_conflicts: IntCounter,
    /// Accounts in the merged state-override snapshot.
    venue_count: IntGauge,
    /// Simulations by whether overrides were applied, and why not if they
    /// weren't.
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
    /// No frame arrived within the configured `max_age`.
    TooOld,
    /// The stream fell behind the block being simulated.
    WrongBlock,
    /// The stream has not published any override yet.
    Empty,
}

impl OverrideResult {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::TooOld => "too_old",
            Self::WrongBlock => "wrong_block",
            Self::Empty => "empty",
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{U256, address},
        alloy_provider::{Provider, network::TransactionBuilder},
        alloy_sol_types::SolValue,
        futures::StreamExt,
        std::time::Duration,
        tokio::time::timeout,
        tokio_tungstenite::tungstenite::Message,
    };

    /// Shared `PrioUpdateRegistry` every venue writes its lanes into.
    const REGISTRY: Address = address!("da7afeed01fe625cf15d187a19f94b45f00b8c5f");
    /// Slot the captured frames below quote for.
    const QUOTED_SLOT: u64 = 14_711_587;

    /// Timestamp of `slot`, which is the stamp lanes quoted for it carry.
    fn quoted_at(slot: u64) -> u32 {
        u32::try_from(BEACON_GENESIS + SLOT_DURATION * slot).unwrap()
    }

    fn frame_with(
        venue: Address,
        account: Address,
        balance: Option<U256>,
        storage_slot: Option<B256>,
    ) -> Frame {
        let mut account_override = AccountOverride::default();
        if let Some(balance) = balance {
            account_override.balance = Some(balance);
        }
        if let Some(storage_slot) = storage_slot {
            let mut diff = B256Map::default();
            diff.insert(storage_slot, B256::ZERO);
            account_override.state_diff = Some(diff);
        }
        let mut state_override = StateOverride::default();
        state_override.insert(account, account_override);
        Frame {
            slot: None,
            block_number: None,
            venues: BTreeMap::from([(venue, VenueUpdate { state_override })]),
        }
    }

    /// A registry word: freshness stamp in the leading bytes, maker price in
    /// the rest.
    fn word(stamp: u32, price: u8) -> B256 {
        let mut word = B256::from([price; 32]);
        word[..STAMP_LEN].copy_from_slice(&stamp.to_be_bytes());
        word
    }

    /// A frame quoting `lanes` of the shared registry on behalf of `venue`.
    fn registry_frame(venue: Address, slot: u64, lanes: &[(B256, B256)]) -> Frame {
        let account_override = AccountOverride {
            state_diff: Some(lanes.iter().copied().collect()),
            ..Default::default()
        };
        let mut state_override = StateOverride::default();
        state_override.insert(REGISTRY, account_override);
        Frame {
            slot: Some(slot),
            block_number: None,
            venues: BTreeMap::from([(venue, VenueUpdate { state_override })]),
        }
    }

    fn lane(index: u8) -> B256 {
        B256::from([index; 32])
    }

    fn lanes_of(overrides: &StateOverride) -> &B256Map<B256> {
        overrides
            .get(&REGISTRY)
            .unwrap()
            .state_diff
            .as_ref()
            .unwrap()
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

    fn fold(frames: Vec<Frame>) -> StateOverride {
        let mut venues = Venues::default();
        for frame in frames {
            venues.update(frame);
        }
        venues.fold().0
    }

    #[test]
    fn a_venues_frame_replaces_its_previous_one() {
        let venue = address!("1111111111111111111111111111111111111111");
        let account = address!("2222222222222222222222222222222222222222");

        // A frame carries the venue's whole set of lanes, so what the newest
        // one leaves out is no longer being quoted and has to fall back to
        // committed on-chain state.
        let overrides = fold(vec![
            frame_with(venue, account, None, Some(B256::ZERO)),
            frame_with(venue, account, Some(U256::ZERO), None),
        ]);
        assert_eq!(overrides[&account].balance, Some(U256::ZERO));
        assert!(overrides[&account].state_diff.is_none());
    }

    #[test]
    fn venues_keep_each_others_accounts() {
        let venue_a = address!("1111111111111111111111111111111111111111");
        let venue_b = address!("3333333333333333333333333333333333333333");
        let account_a = address!("2222222222222222222222222222222222222222");
        let account_b = address!("4444444444444444444444444444444444444444");

        let overrides = fold(vec![
            frame_with(venue_a, account_a, None, Some(B256::ZERO)),
            frame_with(venue_b, account_b, None, Some(B256::ZERO)),
        ]);
        assert!(overrides.contains_key(&account_a));
        assert!(overrides.contains_key(&account_b));
    }

    #[test]
    fn a_venue_omitting_a_field_does_not_clear_anothers() {
        let venue_a = address!("1111111111111111111111111111111111111111");
        let venue_b = address!("3333333333333333333333333333333333333333");
        let shared = address!("2222222222222222222222222222222222222222");

        // Venue B describes the shared account without a balance, which means
        // "unchanged", not "cleared": it only ever states what it set itself.
        let overrides = fold(vec![
            frame_with(venue_a, shared, Some(U256::from(1)), None),
            frame_with(venue_b, shared, None, Some(B256::ZERO)),
        ]);
        assert_eq!(overrides[&shared].balance, Some(U256::from(1)));
        assert!(overrides[&shared].state_diff.is_some());
    }

    fn handle(receiver: watch::Receiver<Snapshot>, max_age: Duration) -> SimulationOverrides {
        SimulationOverrides(Arc::new(Inner {
            snapshots: receiver,
            max_age,
        }))
    }

    /// Handle over a snapshot built by folding `frames` in order.
    fn handle_for(frames: Vec<Frame>, block_number: u64, max_age: Duration) -> SimulationOverrides {
        let mut venues = Venues::default();
        for frame in frames {
            venues.update(frame);
        }
        let (overrides, stamp) = venues.fold();
        let (_sender, receiver) = watch::channel(Snapshot {
            overrides,
            block_number,
            stamp,
            received_at: Some(Instant::now()),
        });
        handle(receiver, max_age)
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
            stamp: None,
            received_at: Some(received_at),
        }
    }

    #[test]
    fn overrides_withheld_when_stream_fell_behind() {
        // No frame arrived recently enough.
        let (_sender, receiver) = watch::channel(non_empty_snapshot(
            100,
            Instant::now() - Duration::from_millis(100),
        ));
        assert!(
            handle(receiver, Duration::from_millis(50))
                .overrides_for(99, 1000)
                .is_none()
        );

        // The stream fell behind: it still describes block 100 while block 105
        // is being simulated, so the maker has moved on from its price.
        let (_sender, receiver) = watch::channel(non_empty_snapshot(100, Instant::now()));
        assert!(
            handle(receiver, Duration::from_secs(30))
                .overrides_for(105, 1000)
                .is_none()
        );
    }

    #[test]
    fn overrides_withheld_when_nothing_published() {
        // Nothing published yet.
        let (_sender, receiver) = watch::channel(Snapshot {
            overrides: StateOverride::default(),
            block_number: 100,
            stamp: None,
            received_at: Some(Instant::now()),
        });
        assert!(
            handle(receiver, Duration::from_secs(30))
                .overrides_for(99, 1000)
                .is_none()
        );
    }

    #[test]
    fn snapshot_ahead_of_simulated_block_is_served() {
        // Frames name the block the builder is about to build, so the snapshot
        // being one ahead of the simulated block is the normal case.
        let (_sender, receiver) = watch::channel(non_empty_snapshot(100, Instant::now()));
        assert!(
            handle(receiver, Duration::from_secs(30))
                .overrides_for(99, 1000)
                .is_some()
        );
    }

    #[test]
    fn quoted_lanes_are_restamped_to_the_simulated_block() {
        let venue = address!("1111111111111111111111111111111111111111");
        let stamp = quoted_at(QUOTED_SLOT);
        let simulated_at = stamp - 12;

        let handle = handle_for(
            vec![registry_frame(
                venue,
                QUOTED_SLOT,
                &[(lane(1), word(stamp, 0xaa)), (lane(2), word(stamp, 0xbb))],
            )],
            100,
            Duration::from_secs(30),
        );

        let overrides = handle.overrides_for(99, simulated_at.into()).unwrap();
        let lanes = lanes_of(&overrides);
        assert_eq!(lanes[&lane(1)], word(simulated_at, 0xaa));
        assert_eq!(lanes[&lane(2)], word(simulated_at, 0xbb));
    }

    #[test]
    fn lane_not_requoted_keeps_its_stale_stamp() {
        let venue = address!("1111111111111111111111111111111111111111");
        let stamp = quoted_at(QUOTED_SLOT);
        let previous = quoted_at(QUOTED_SLOT - 1);
        let simulated_at = stamp - 12;

        // The venue still carries lane 2 in its frame, but stamped for the
        // previous slot: it is not quoting that pair for this block.
        let handle = handle_for(
            vec![registry_frame(
                venue,
                QUOTED_SLOT,
                &[
                    (lane(1), word(stamp, 0xcc)),
                    (lane(2), word(previous, 0xbb)),
                ],
            )],
            100,
            Duration::from_secs(30),
        );

        let overrides = handle.overrides_for(99, simulated_at.into()).unwrap();
        let lanes = lanes_of(&overrides);
        assert_eq!(lanes[&lane(1)], word(simulated_at, 0xcc));
        // Lane 2 must stay dead: forging liveness for it would quote a price
        // the maker is no longer offering.
        assert_eq!(lanes[&lane(2)], word(previous, 0xbb));
    }

    #[test]
    fn restamping_leaves_every_other_byte_untouched() {
        let mut venues = Venues::default();
        venues.update(serde_json::from_str(FERMI_FRAME).unwrap());
        let (overrides, stamp) = venues.fold();
        assert_eq!(stamp, Some(quoted_at(QUOTED_SLOT)));

        let simulated_at = quoted_at(QUOTED_SLOT) - 12;
        let mut restamped = overrides.clone();
        let overrides = restamp(overrides, stamp, simulated_at.into());

        // Only the stamp bytes of the registry words moved; the maker's price
        // bytes and every other account are byte-identical.
        let (before, after) = (lanes_of(&overrides), lanes_of(&restamped));
        assert_eq!(before.len(), after.len());
        for (slot, before) in before {
            let after = after[slot];
            assert_eq!(&after[..STAMP_LEN], &simulated_at.to_be_bytes());
            assert_eq!(after[STAMP_LEN..], before[STAMP_LEN..]);
        }
        for (account, before) in &overrides {
            if *account != REGISTRY {
                assert_eq!(&restamped[account], before);
            }
        }
    }

    #[test]
    fn unrelated_words_are_never_mistaken_for_a_stamp() {
        // This venue's word leads with bytes that read as a perfectly plausible
        // unix timestamp (2019-07-25) — just not the one its slot is quoting
        // for, so the frame is recognised as quoting no lane at all and the
        // word survives the accessor untouched.
        let frame: Frame = serde_json::from_str(OTHER_FRAME).unwrap();
        let venue = address!("28d9ccedf1b7ac9b3f090f4f0292837de87c1d39");
        let mut venues = Venues::default();
        venues.update(frame);
        assert_eq!(venues.fold().1, None);

        let handle = handle_for(
            vec![serde_json::from_str(OTHER_FRAME).unwrap()],
            100,
            Duration::from_secs(30),
        );
        let simulated_at = quoted_at(QUOTED_SLOT) - 12;
        let overrides = handle.overrides_for(99, simulated_at.into()).unwrap();
        assert_eq!(overrides[&venue], venues.fold().0[&venue]);
    }

    #[test]
    fn venues_sharing_the_registry_keep_each_others_lanes() {
        let venue_a = address!("1111111111111111111111111111111111111111");
        let venue_b = address!("3333333333333333333333333333333333333333");
        let stamp = quoted_at(QUOTED_SLOT);
        let simulated_at = stamp - 12;

        // Both venues write the *same* registry account, each frame carrying
        // only its own lanes, so overriding the account per frame would drop
        // the other's.
        let handle = handle_for(
            vec![
                registry_frame(venue_a, QUOTED_SLOT, &[(lane(1), word(stamp, 0xaa))]),
                registry_frame(venue_b, QUOTED_SLOT, &[(lane(2), word(stamp, 0xbb))]),
            ],
            100,
            Duration::from_secs(30),
        );

        let overrides = handle.overrides_for(99, simulated_at.into()).unwrap();
        let lanes = lanes_of(&overrides);
        assert_eq!(lanes.len(), 2);
        assert_eq!(lanes[&lane(1)], word(simulated_at, 0xaa));
        assert_eq!(lanes[&lane(2)], word(simulated_at, 0xbb));
    }

    #[tokio::test]
    async fn reconnects_and_serves_newest_frame_from_in_process_server() {
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

        let cfg = Config {
            ws_url: server_url,
            max_age: Duration::from_secs(30),
        };
        let handle = spawn(&cfg);

        let _ = server_handle.await;

        let got = timeout(Duration::from_secs(2), async {
            loop {
                // The frames name block 11, the block the builder is about to
                // build on top of head 10.
                if let Some(overrides) = handle.overrides_for(10, 1000)
                    && let Some(account_override) = overrides.get(&account)
                    && account_override.balance == Some(U256::from(2))
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;
        assert!(got.is_ok(), "did not observe the newest overrides in time");
    }

    // Real frames captured from wss://eu.rpc.titanbuilder.xyz/ws/pamm_quote_stream.
    const FERMI_FRAME: &str = r#"{
        "slot":14711587,
        "blockNumber":25475333,
        "timestamp":1783363067584411872,
        "0xb1076fe3ab5e28005c7c323bac5ac06a680d452e":{
            "stateOverride":{
                "0xa048e0c08b7acb48363711800ac9d49de8e58d13":{
                    "balance":"0x10848dc44f1e140",
                    "nonce":"0x42ee"
                },
                "0x14e870f0a7c764ca71289952006d6bf130058927":{
                    "balance":"0x10aaf167cb7dbea",
                    "nonce":"0x23da"
                },
                "0x69939a6c590c9cd0bf8efbe9b3df2cdac4a4906b":{
                    "balance":"0x88ecb0471d376e",
                    "nonce":"0x2fad"
                },
                "0xfc42be9494f1af6b03adad71811c62ada2d6f3c3":{
                    "balance":"0x10e7e3c0b4f8ba0",
                    "nonce":"0x1d99"
                },
                "0xda7afeed01fe625cf15d187a19f94b45f00b8c5f":{
                    "balance":"0x0",
                    "nonce":"0x1",
                    "stateDiff":{
                        "0x6d3af688dd77e4167e6ad8613dea4a162f5e340043b2c026e3d2b5b40d12c92d":"0x6a4bf5fb010000000000000000000000000000000000000000000029cdbee960",
                        "0x9a965d2bccf7f891d58fe85acac20d9de58c11ac1d222dfff7973a09ad71143a":"0x6a4bf5fb010000000000000000000000000000000000000000000029c8581698",
                        "0xe25ff9533ce41163d3738b63c7d954cd7449a0ba0dd0dac8db25ae29536b4961":"0x6a4bf5fb010000000000000000000000000000000000000000000029cdbee960",
                        "0x939ee2e42000f154d3be2302ab4d3cb916e4b2852ef6a0caa2fd76c417120248":"0x6a4bf5fb010000000000000000000000000000000000000000000029c8581698"
                    }
                }
            }
        }
    }"#;

    const OTHER_FRAME: &str = r#"{
        "slot":14711587,
        "blockNumber":25475333,
        "timestamp":1783363067506613546,
        "0x28d9ccedf1b7ac9b3f090f4f0292837de87c1d39":{
            "stateOverride":{
                "0x28d9ccedf1b7ac9b3f090f4f0292837de87c1d39":{
                    "balance":"0x0",
                    "nonce":"0x1",
                    "stateDiff":{
                        "0xe3ffa73f3a3b56e693c2ed775464cb3fbe78307b000000000000000000000000":"0x5d393a1348485d39e6f3484885cda948444485cd9644363600019f38b8de3300"
                    }
                },
                "0xe3ffa73f3a3b56e693c2ed775464cb3fbe78307b":{
                    "balance":"0x12e8705b8c388ab1",
                    "nonce":"0xdda1"
                }
            }
        }
    }"#;

    #[test]
    fn parses_real_titan_frames() {
        let fermi: Frame = serde_json::from_str(FERMI_FRAME).unwrap();
        assert_eq!(fermi.block_number, Some(25475333));
        assert_eq!(fermi.slot, Some(QUOTED_SLOT));
        assert_eq!(fermi.venues.len(), 1);

        let mut venues = Venues::default();
        venues.update(fermi);
        let (overrides, stamp) = venues.fold();
        assert_eq!(stamp, Some(quoted_at(QUOTED_SLOT)));
        assert_eq!(overrides.len(), 5);

        let entry = &overrides[&REGISTRY];
        assert_eq!(entry.balance, Some(U256::ZERO));
        assert_eq!(entry.nonce, Some(1));
        let diff = entry.state_diff.as_ref().unwrap();
        assert_eq!(diff.len(), 4);
        let slot: B256 = "0x6d3af688dd77e4167e6ad8613dea4a162f5e340043b2c026e3d2b5b40d12c92d"
            .parse()
            .unwrap();
        assert!(diff.contains_key(&slot));
    }

    #[test]
    fn a_lanes_stamp_is_the_timestamp_of_the_slot_its_frame_quotes_for() {
        // The whole restamping rule rests on this: the leading bytes of a
        // freshly quoted lane are the slot's timestamp, so which words are
        // stamps never has to be guessed from their contents.
        let mut venues = Venues::default();
        venues.update(serde_json::from_str(FERMI_FRAME).unwrap());
        let overrides = venues.fold().0;
        let stamp = quoted_at(QUOTED_SLOT).to_be_bytes();
        assert!(
            lanes_of(&overrides)
                .values()
                .all(|word| word[..STAMP_LEN] == stamp)
        );
    }

    #[test]
    fn real_frames_of_different_venues_are_folded_together() {
        let overrides = fold(vec![
            serde_json::from_str(FERMI_FRAME).unwrap(),
            serde_json::from_str(OTHER_FRAME).unwrap(),
        ]);

        assert_eq!(overrides.len(), 7);
        assert!(overrides.contains_key(&REGISTRY));
        assert!(overrides.contains_key(&address!("e3ffa73f3a3b56e693c2ed775464cb3fbe78307b")));
    }

    #[test]
    fn yields_state_overrides_for_real_data() {
        let handle = handle_for(
            vec![serde_json::from_str(FERMI_FRAME).unwrap()],
            25475333,
            Duration::from_secs(30),
        );

        // The frame names 25475333, so it serves a simulation on head 25475332.
        let head_timestamp = u64::from(quoted_at(QUOTED_SLOT)) - 12;
        let overrides = handle.overrides_for(25475332, head_timestamp).unwrap();
        assert_eq!(overrides.len(), 5);

        // ...but not one on a block the stream has already fallen behind.
        assert!(
            handle
                .overrides_for(25475335, head_timestamp + 36)
                .is_none()
        );
    }

    /// Titan's Fermi router. Its pAMM reverts `StaleUpdate()` unless the
    /// registry word it reads carries the timestamp of the block the call runs
    /// against, which is exactly what restamping provides.
    const FERMI_ROUTER: Address = address!("b1076fe3ab5e28005c7c323bac5ac06a680d452e");
    const USDT: Address = address!("dac17f958d2ee523a2206206994597c13d831ec7");
    const WETH: Address = address!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2");

    /// `(address tokenIn, address tokenOut, uint256 amountIn) view` on the
    /// Fermi router. The contract is unverified, so the quote is addressed by
    /// selector rather than by name.
    const QUOTE: [u8; 4] = hex_literal::hex!("300aa47f");

    /// The workspace links more than one `rustls` crypto provider, so one has
    /// to be chosen before any TLS handshake. The binaries get this for free:
    /// the alloy websocket transport installs a provider while opening the
    /// block stream, long before this stream connects. A test process does
    /// not, so it picks one itself — and tolerates a provider already being
    /// installed, which is what happens once anything else in the process has
    /// opened a TLS connection.
    fn install_crypto_provider() {
        let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    }

    fn live_stream_config() -> Config {
        Config {
            ws_url: std::env::var("PAMM_QUOTE_STREAM_URL")
                .unwrap()
                .parse()
                .unwrap(),
            max_age: Duration::from_secs(30),
        }
    }

    /// Quotes 1000 USDT into WETH, returning the amount out.
    async fn quote_amounts(
        provider: &ethrpc::AlloyProvider,
        block: u64,
        overrides: Option<StateOverride>,
    ) -> Result<U256, alloy_transport::TransportError> {
        let args = (USDT, WETH, U256::from(1_000_000_000u64));
        let output = provider
            .call(
                alloy_rpc_types::TransactionRequest::default()
                    .with_to(FERMI_ROUTER)
                    .with_input([QUOTE.as_slice(), &args.abi_encode_params()].concat()),
            )
            .overrides_opt(overrides)
            .block(block.into())
            .await?;
        let (_amount_in, amount_out) = <(U256, U256)>::abi_decode_params(&output).unwrap();
        Ok(amount_out)
    }

    /// Exercises the gas path end to end: `Simulator::gas` must estimate a pAMM
    /// call that only succeeds when the live overrides are applied in the
    /// context they were stamped for.
    ///
    /// Also pins down why the estimate is no longer run against `pending`: the
    /// very same overrides are rejected there, because `pending`'s timestamp is
    /// the node's wall clock rather than the block they were stamped for.
    ///
    /// See `crates/simulator/README.md` for how to run this.
    #[tokio::test]
    #[ignore]
    async fn pamm_estimates_gas_for_call() {
        install_crypto_provider();

        let web3 = ethrpc::Web3::new_from_env();
        let ws_url: url::Url = std::env::var("NODE_WS_URL").unwrap().parse().unwrap();
        let blocks = ethrpc::block_stream::current_block_ws_stream(web3.provider.clone(), ws_url)
            .await
            .unwrap();
        let overrides = super::spawn(&live_stream_config());
        tokio::time::sleep(Duration::from_secs(5)).await;

        let eth = crate::Ethereum::new(
            web3.clone(),
            chain::Chain::Mainnet,
            Default::default(),
            Arc::new(gas_price_estimation::FakeGasPriceEstimator::default()),
            blocks.clone(),
            U256::from(30_000_000),
        );
        let args = (USDT, WETH, U256::from(1_000_000_000u64));
        let tx = eth_domain_types::Tx {
            from: Address::ZERO,
            to: FERMI_ROUTER,
            value: U256::ZERO.into(),
            input: [QUOTE.as_slice(), &args.abi_encode_params()]
                .concat()
                .into(),
            access_list: Default::default(),
        };

        // Without the stream the venue is stale and the estimate reverts.
        let bare = crate::Simulator::ethereum(eth.clone());
        assert!(
            bare.gas(tx.clone()).await.is_err(),
            "estimate succeeded without overrides, the check proves nothing"
        );

        // With it, the same estimate goes through.
        let mut simulator = crate::Simulator::ethereum(eth);
        simulator.set_simulation_overrides(overrides.clone());
        let gas = simulator
            .gas(tx.clone())
            .await
            .expect("gas estimation reverted with overrides applied");
        assert!(
            gas.0 > U256::from(21_000),
            "implausible gas estimate {gas:?}"
        );

        // ...but not against `pending`, which is where it used to run.
        let head = *blocks.borrow();
        let state = overrides
            .overrides_for(head.number, head.timestamp)
            .expect("stream served no overrides at head");
        let request = alloy_rpc_types::TransactionRequest::default()
            .with_to(FERMI_ROUTER)
            .with_input(tx.input.0.clone());
        assert!(
            web3.provider
                .estimate_gas(request)
                .overrides(state)
                .pending()
                .await
                .is_err(),
            "overrides stamped for {} were accepted at pending",
            head.number
        );
    }

    /// The overrides have to be served continuously, not just in the sliver
    /// right after a block lands. Samples the accessor at chain head across
    /// several blocks and requires virtually all samples to be served; the
    /// block-number gate this replaced scored about 5% here.
    ///
    /// See `crates/simulator/README.md` for how to run this.
    #[tokio::test]
    #[ignore]
    async fn pamm_serves_overrides_across_blocks() {
        install_crypto_provider();

        // A real block watcher, as the binaries build it.
        let provider = ethrpc::Web3::new_from_env().provider;
        let ws_url: url::Url = std::env::var("NODE_WS_URL").unwrap().parse().unwrap();
        let blocks = ethrpc::block_stream::current_block_ws_stream(provider, ws_url)
            .await
            .unwrap();

        let handle = super::spawn(&live_stream_config());
        tokio::time::sleep(Duration::from_secs(5)).await;

        // ~60s, several blocks' worth.
        let (mut served, mut withheld) = (0u32, 0u32);
        let mut blocks_seen = std::collections::BTreeSet::new();
        for _ in 0..600 {
            let head = *blocks.borrow();
            blocks_seen.insert(head.number);
            match handle.overrides_for(head.number, head.timestamp) {
                Some(_) => served += 1,
                None => withheld += 1,
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        assert!(
            blocks_seen.len() >= 2,
            "block watcher never advanced, the sample spans no block boundary"
        );
        assert!(
            served >= (served + withheld) * 9 / 10,
            "overrides served for only {served} of {} samples across {} blocks",
            served + withheld,
            blocks_seen.len()
        );
    }

    /// Live conformance check: the overrides this module hands out must
    /// actually make a Titan pAMM quote, in the exact context they are applied
    /// in. Nothing short of a real stream against a real node catches a frame
    /// that describes a different block than the one being simulated.
    #[tokio::test]
    #[ignore]
    async fn pamm_quotes_against_live_stream() {
        install_crypto_provider();

        let provider = ethrpc::Web3::new_from_env().provider;
        let overrides = super::spawn(&live_stream_config());

        let quoted = timeout(Duration::from_secs(60), async {
            loop {
                let head =
                    ethrpc::block_stream::get_block_at_id(&provider, alloy_eips::BlockId::latest())
                        .await
                        .unwrap();
                if let Some(state) = overrides.overrides_for(head.number, head.timestamp) {
                    // Without the overrides the pool has nothing fresh to
                    // quote from and reverts `StaleUpdate()` (0x666a2814);
                    // with them it quotes the maker's live price.
                    assert!(
                        quote_amounts(&provider, head.number, None).await.is_err(),
                        "venue quoted without overrides, the check proves nothing"
                    );
                    let amount_out = quote_amounts(&provider, head.number, Some(state))
                        .await
                        .expect("pAMM quote reverted with overrides applied");
                    assert!(amount_out > U256::ZERO, "venue quoted nothing");
                    return;
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
        .await;
        assert!(quoted.is_ok(), "stream never served the chain head");
    }
}
