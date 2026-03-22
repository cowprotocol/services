use {
    crate::{
        boundary::{self, SolvableOrders},
        domain::{self, auction::Price},
        infra::{self, banned},
    },
    account_balances::{BalanceFetching, Query},
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    bad_tokens::list_based::DenyListedTokens,
    database::order_events::{
        OrderEventLabel,
        OrderFilterReason::{
            self,
            BannedUser,
            DustOrder,
            InFlight,
            InsufficientBalance,
            InvalidSignature,
            MissingNativePrice,
            UnsupportedToken,
        },
    },
    futures::FutureExt,
    itertools::Itertools,
    model::{
        order::{Order, OrderClass, OrderUid},
        signature::Signature,
        time::now_in_epoch_seconds,
    },
    price_estimation::{
        native::{NativePriceEstimating, to_normalized_price},
        native_price_cache::NativePriceUpdater,
    },
    prometheus::{Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec},
    sha2::{Digest, Sha256},
    shared::remaining_amounts,
    std::{
        cmp::Ordering,
        collections::{BTreeMap, HashMap, HashSet, VecDeque, btree_map::Entry},
        future::Future,
        sync::Arc,
        time::{Duration, Instant},
    },
    strum::VariantNames,
    tokio::sync::{Mutex, broadcast},
    tracing::instrument,
};
#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Tracks success and failure of the solvable orders cache update task.
    #[metric(labels("result"))]
    auction_update: IntCounterVec,

    /// Time taken to update the solvable orders cache.
    #[metric(buckets(
        0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.25, 2.5, 2.75, 3.0, 3.5, 4, 5
    ))]
    auction_update_total_time: Histogram,

    /// Time spent on auction update individual stage.
    #[metric(
        labels("stage"),
        buckets(
            0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0, 5.0
        )
    )]
    auction_update_stage_time: HistogramVec,

    /// Auction creations.
    auction_creations: IntCounter,

    /// Auction candidate orders grouped by class.
    #[metric(labels("class"))]
    auction_candidate_orders: IntGaugeVec,

    /// Auction solvable orders grouped by class.
    #[metric(labels("class"))]
    auction_solvable_orders: IntGaugeVec,

    /// Auction filtered orders grouped by class.
    #[metric(labels("reason"))]
    auction_filtered_orders: IntGaugeVec,

    /// Auction filtered market orders due to missing native token price.
    auction_market_order_missing_price: IntGauge,
    /// Delta incremental shadow comparison result.
    #[metric(labels("result"))]
    delta_shadow_compare: IntCounterVec,

    /// Delta incremental-primary rollout result.
    #[metric(labels("result"))]
    delta_incremental_primary: IntCounterVec,

    /// Number of canonical delta fallbacks due to mismatched incremental diffs.
    delta_canonical_fallback_total: IntCounter,

    /// Total incremental delta comparisons.
    delta_incremental_event_total: IntCounter,

    /// Incremental delta comparisons that mismatched the canonical diff.
    delta_incremental_event_mismatch_total: IntCounter,

    /// Incremental delta filter transitions that referenced a missing order.
    delta_filter_transition_missing_order_total: IntCounter,

    /// Incremental projection mismatches against the canonical rebuild surface.
    delta_incremental_projection_mismatch_total: IntCounter,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    #[instrument(skip_all)]
    fn track_filtered_orders(reason: OrderFilterReason, invalid_orders: &[OrderUid]) {
        if invalid_orders.is_empty() {
            return;
        }

        Metrics::get()
            .auction_filtered_orders
            .with_label_values(&[reason.as_str()])
            .set(i64::try_from(invalid_orders.len()).unwrap_or(i64::MAX));

        tracing::debug!(
            %reason,
            count = invalid_orders.len(),
            orders = ?invalid_orders, "filtered orders"
        );
    }

    #[instrument(skip_all)]
    fn track_orders_in_final_auction(orders: &[&Order]) {
        let metrics = Metrics::get();
        metrics.auction_creations.inc();

        let remaining_counts = orders
            .iter()
            .counts_by(|order| order.metadata.class.as_ref());
        for class in OrderClass::VARIANTS {
            let count = remaining_counts.get(class).copied().unwrap_or_default();
            metrics
                .auction_solvable_orders
                .with_label_values(&[class])
                .set(i64::try_from(count).unwrap_or(i64::MAX));
        }
    }
}

/// Keeps track and updates the set of currently solvable orders.
/// For this we also need to keep track of user sell token balances for open
/// orders so this is retrievable as well.
/// The cache is updated in the background whenever a new block appears or when
/// the cache is explicitly notified that it should update for example because a
/// new order got added to the order book.
pub struct SolvableOrdersCache {
    min_order_validity_period: Duration,
    persistence: infra::Persistence,
    banned_users: banned::Users,
    balance_fetcher: Arc<dyn BalanceFetching>,
    deny_listed_tokens: DenyListedTokens,
    cache: Mutex<Option<Inner>>,
    update_lock: Mutex<()>,
    native_price_estimator: Arc<NativePriceUpdater>,
    weth: Address,
    protocol_fees: domain::ProtocolFees,
    cow_amm_registry: cow_amm::Registry,
    native_price_timeout: Duration,
    settlement_contract: Address,
    disable_order_balance_filter: bool,
    wrapper_cache: app_data::WrapperCache,
    delta_sender: broadcast::Sender<DeltaEnvelope>,
    shadow_compare_incremental: bool,
    incremental_primary: bool,
}

type Balances = HashMap<Query, U256>;

const MAX_DELTA_HISTORY: usize = 1024;
const DEFAULT_DELTA_HISTORY_MAX_AGE: Duration = Duration::from_secs(300);
const MIN_DELTA_HISTORY_RETAINED: usize = 10;
const DELTA_BROADCAST_CAPACITY: usize = MAX_DELTA_HISTORY;

#[derive(Clone, Debug, PartialEq)]
pub enum DeltaEvent {
    /// Advisory marker for auction boundary transitions.
    AuctionChanged {
        new_auction_id: u64,
    },
    /// Order entered the solver-visible set (including
    /// re-validation/unfiltering).
    OrderAdded(domain::Order),
    /// Order left the solver-visible set (including invalidation/filtering).
    OrderRemoved(domain::OrderUid),
    OrderUpdated(domain::Order),
    PriceChanged {
        token: Address,
        price: Option<Price>,
    },
}

#[derive(Clone, Debug)]
pub struct DeltaEnvelope {
    pub auction_id: u64,
    pub auction_sequence: u64,
    pub from_sequence: u64,
    pub to_sequence: u64,
    pub published_at: chrono::DateTime<chrono::Utc>,
    /// Monotonic timestamp used only for in-memory pruning.
    pub published_at_instant: Instant,
    pub events: Vec<DeltaEvent>,
}

#[derive(Clone, Debug)]
pub struct DeltaSnapshot {
    pub auction_id: u64,
    pub auction_sequence: u64,
    pub sequence: u64,
    pub oldest_available: u64,
    pub auction: domain::RawAuctionData,
}

#[derive(Clone, Debug)]
pub struct DeltaChecksum {
    pub sequence: u64,
    pub order_uid_hash: String,
    pub price_hash: String,
}

#[derive(Clone, Copy, Debug)]
pub enum DeltaAfterError {
    FutureSequence { latest: u64 },
    ResyncRequired { oldest_available: u64, latest: u64 },
}

#[derive(Clone, Copy, Debug)]
pub enum DeltaSubscribeError {
    MissingAfterSequence { latest: u64 },
    DeltaAfter(DeltaAfterError),
}

#[derive(Clone, Debug)]
pub struct DeltaReplay {
    pub checkpoint_sequence: u64,
    pub envelopes: Vec<DeltaEnvelope>,
}

struct Inner {
    auction: domain::RawAuctionData,
    solvable_orders: boundary::SolvableOrders,
    auction_id: u64,
    auction_sequence: u64,
    delta_sequence: u64,
    delta_history: VecDeque<DeltaEnvelope>,
    indexed_state: Arc<IndexedAuctionState>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct IndexedAuctionState {
    current_orders_by_uid: HashMap<domain::OrderUid, domain::Order>,
    current_prices_by_token: HashMap<Address, Price>,
    filtered_invalid: HashSet<OrderUid>,
    filtered_in_flight: HashSet<OrderUid>,
    filtered_no_balance: HashSet<OrderUid>,
    filtered_no_price: HashSet<OrderUid>,
}

#[derive(Clone, Debug, Default)]
struct ChangeBundle {
    order_added_candidates: Vec<domain::OrderUid>,
    order_removed_candidates: Vec<domain::OrderUid>,
    order_updated_candidates: Vec<domain::OrderUid>,
    quote_updated_candidates: Vec<domain::OrderUid>,
    price_changed_tokens: Vec<Address>,
    filter_transitions: Vec<FilterTransition>,
}

#[derive(Clone, Debug)]
struct FilterTransition {
    uid: domain::OrderUid,
    reason: OrderFilterReason,
    is_filtered: bool,
}

struct CollectedAuctionInputs {
    db_solvable_orders: boundary::SolvableOrders,
    invalid_order_uids: HashMap<OrderUid, OrderFilterReason>,
    filtered_order_events: Vec<(OrderUid, OrderFilterReason)>,
    final_orders: Vec<domain::Order>,
    prices: BTreeMap<Address, U256>,
    surplus_capturing_jit_order_owners: Vec<Address>,
    indexed_state: Option<Arc<IndexedAuctionState>>,
}

impl SolvableOrdersCache {
    #[expect(clippy::too_many_arguments)]
    pub fn new(
        min_order_validity_period: Duration,
        persistence: infra::Persistence,
        banned_users: banned::Users,
        balance_fetcher: Arc<dyn BalanceFetching>,
        deny_listed_tokens: DenyListedTokens,
        native_price_estimator: Arc<NativePriceUpdater>,
        weth: Address,
        protocol_fees: domain::ProtocolFees,
        cow_amm_registry: cow_amm::Registry,
        native_price_timeout: Duration,
        settlement_contract: Address,
        disable_order_balance_filter: bool,
    ) -> Arc<Self> {
        let (delta_sender, _) = broadcast::channel(DELTA_BROADCAST_CAPACITY);
        let shadow_compare_incremental = shared::env::flag_enabled(
            std::env::var("AUTOPILOT_DELTA_SYNC_SHADOW_COMPARE")
                .ok()
                .as_deref(),
            false,
        );
        let incremental_primary = shared::env::flag_enabled(
            std::env::var("AUTOPILOT_DELTA_SYNC_INCREMENTAL_PRIMARY")
                .ok()
                .as_deref(),
            false,
        );
        if shadow_compare_incremental && incremental_primary {
            tracing::warn!(
                "delta sync shadow-compare and incremental-primary are both enabled; updates will \
                 run the full pipeline twice"
            );
        }
        Arc::new(Self {
            min_order_validity_period,
            persistence,
            banned_users,
            balance_fetcher,
            deny_listed_tokens,
            cache: Mutex::new(None),
            update_lock: Mutex::new(()),
            native_price_estimator,
            weth,
            protocol_fees,
            cow_amm_registry,
            native_price_timeout,
            settlement_contract,
            disable_order_balance_filter,
            wrapper_cache: app_data::WrapperCache::new(20_000),
            delta_sender,
            shadow_compare_incremental,
            incremental_primary,
        })
    }

    /// Debug-only guard to catch cache mutation without the update lock.
    fn assert_update_lock_held(&self) {
        #[cfg(debug_assertions)]
        if let Ok(guard) = self.update_lock.try_lock() {
            // If try_lock succeeds, the update lock was not held by the caller.
            drop(guard);
            debug_assert!(false, "update_lock must be held when mutating cache");
        }
    }

    pub async fn current_auction(&self) -> Option<domain::RawAuctionData> {
        self.cache
            .lock()
            .await
            .as_ref()
            .map(|inner| inner.auction.clone())
    }

    pub async fn delta_snapshot(&self) -> Option<DeltaSnapshot> {
        self.cache.lock().await.as_ref().map(|inner| {
            let oldest_available = inner
                .delta_history
                .front()
                .map(|envelope| envelope.from_sequence)
                .unwrap_or(inner.delta_sequence);
            DeltaSnapshot {
                auction_id: inner.auction_id,
                auction_sequence: inner.auction_sequence,
                sequence: inner.delta_sequence,
                oldest_available,
                auction: inner.auction.clone(),
            }
        })
    }

    pub async fn delta_sequence(&self) -> Option<u64> {
        self.cache
            .lock()
            .await
            .as_ref()
            .map(|inner| inner.delta_sequence)
    }

    pub async fn delta_checksum(&self) -> Option<DeltaChecksum> {
        let lock = self.cache.lock().await;
        let inner = lock.as_ref()?;
        Some(DeltaChecksum {
            sequence: inner.delta_sequence,
            order_uid_hash: checksum_order_uids(&inner.auction.orders),
            price_hash: checksum_prices(&inner.auction.prices),
        })
    }

    #[cfg(test)]
    pub(crate) async fn set_state_for_tests(
        &self,
        auction: domain::RawAuctionData,
        auction_id: u64,
        auction_sequence: u64,
        delta_sequence: u64,
        delta_history: VecDeque<DeltaEnvelope>,
    ) {
        let indexed_state = Arc::new(build_indexed_state(&auction, &HashMap::new(), &Vec::new()));
        let mut lock = self.cache.lock().await;
        *lock = Some(Inner {
            auction,
            solvable_orders: boundary::SolvableOrders {
                orders: HashMap::new(),
                quotes: HashMap::new(),
                latest_settlement_block: 0,
                fetched_from_db: chrono::Utc::now(),
            },
            auction_id,
            auction_sequence,
            delta_sequence,
            delta_history,
            indexed_state,
        });
    }

    #[cfg(test)]
    pub(crate) async fn publish_delta_for_tests(&self, envelope: DeltaEnvelope) {
        let mut lock = self.cache.lock().await;
        if let Some(inner) = lock.as_mut() {
            inner.auction_id = envelope.auction_id;
            inner.auction_sequence = envelope.auction_sequence;
            inner.delta_sequence = envelope.to_sequence;
            inner.delta_history.push_back(envelope.clone());
            let max_age = chrono::Duration::from_std(delta_history_max_age())
                .unwrap_or_else(|_| chrono::Duration::seconds(60));
            prune_delta_history(&mut inner.delta_history, max_age);
        }
        let _ = self.delta_sender.send(envelope);
    }

    pub async fn set_auction_id(&self, auction_id: u64) {
        let _update_guard = self.update_lock.lock().await;
        self.assert_update_lock_held();
        let mut lock = self.cache.lock().await;
        if let Some(inner) = lock.as_mut() {
            if let Some(envelope) = apply_auction_id_change(inner, auction_id) {
                // Keep replay and live streams aligned by sending while the cache lock is held.
                let _ = self.delta_sender.send(envelope);
            }
        }
    }

    pub async fn delta_after(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<DeltaEnvelope>, DeltaAfterError> {
        self.delta_replay_after(after_sequence).await
    }

    pub async fn delta_replay_after(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<DeltaEnvelope>, DeltaAfterError> {
        Ok(self
            .delta_replay_with_checkpoint(after_sequence)
            .await?
            .envelopes)
    }

    pub async fn delta_replay_with_checkpoint(
        &self,
        after_sequence: u64,
    ) -> Result<DeltaReplay, DeltaAfterError> {
        let lock = self.cache.lock().await;
        Self::build_delta_replay(after_sequence, lock.as_ref())
    }

    pub async fn subscribe_deltas_with_replay(
        &self,
        after_sequence: u64,
    ) -> Result<(broadcast::Receiver<DeltaEnvelope>, DeltaReplay), DeltaAfterError> {
        let lock = self.cache.lock().await;
        let receiver = self.delta_sender.subscribe();
        let replay = Self::build_delta_replay(after_sequence, lock.as_ref())?;
        Ok((receiver, replay))
    }

    pub async fn subscribe_deltas_with_replay_checked(
        &self,
        after_sequence: Option<u64>,
    ) -> Result<(broadcast::Receiver<DeltaEnvelope>, DeltaReplay), DeltaSubscribeError> {
        let lock = self.cache.lock().await;
        let latest = lock.as_ref().map(|inner| inner.delta_sequence).unwrap_or(0);
        if after_sequence.is_none() && latest > 0 {
            return Err(DeltaSubscribeError::MissingAfterSequence { latest });
        }
        let receiver = self.delta_sender.subscribe();
        let replay = Self::build_delta_replay(after_sequence.unwrap_or_default(), lock.as_ref())
            .map_err(DeltaSubscribeError::DeltaAfter)?;
        Ok((receiver, replay))
    }

    fn build_delta_replay(
        after_sequence: u64,
        inner: Option<&Inner>,
    ) -> Result<DeltaReplay, DeltaAfterError> {
        let Some(inner) = inner else {
            if after_sequence > 0 {
                return Err(DeltaAfterError::ResyncRequired {
                    oldest_available: 0,
                    latest: 0,
                });
            }
            return Ok(DeltaReplay {
                checkpoint_sequence: 0,
                envelopes: vec![DeltaEnvelope {
                    auction_id: 0,
                    auction_sequence: 0,
                    from_sequence: 0,
                    to_sequence: 0,
                    published_at: chrono::Utc::now(),
                    published_at_instant: Instant::now(),
                    events: Vec::new(),
                }],
            });
        };

        let checkpoint_sequence = inner.delta_sequence;

        if after_sequence > checkpoint_sequence {
            return Err(DeltaAfterError::FutureSequence {
                latest: checkpoint_sequence,
            });
        }

        let oldest_available = inner
            .delta_history
            .front()
            .map(|envelope| envelope.from_sequence)
            .unwrap_or(inner.delta_sequence);
        if after_sequence < oldest_available {
            return Err(DeltaAfterError::ResyncRequired {
                oldest_available,
                latest: checkpoint_sequence,
            });
        }

        let envelopes: Vec<_> = inner
            .delta_history
            .iter()
            .filter(|envelope| {
                envelope.to_sequence > after_sequence && envelope.to_sequence <= checkpoint_sequence
            })
            .cloned()
            .collect();

        if envelopes.is_empty() {
            Ok(DeltaReplay {
                checkpoint_sequence,
                envelopes: vec![DeltaEnvelope {
                    auction_id: inner.auction_id,
                    auction_sequence: inner.auction_sequence,
                    from_sequence: after_sequence,
                    to_sequence: after_sequence,
                    published_at: chrono::Utc::now(),
                    published_at_instant: Instant::now(),
                    events: Vec::new(),
                }],
            })
        } else {
            Ok(DeltaReplay {
                checkpoint_sequence,
                envelopes,
            })
        }
    }

    pub fn subscribe_deltas(&self) -> broadcast::Receiver<DeltaEnvelope> {
        self.delta_sender.subscribe()
    }

    /// Manually update solvable orders. Usually called by the background
    /// updating task.
    ///
    /// Usually this method is called from update_task. If it isn't, which is
    /// the case in unit tests, then concurrent calls might overwrite each
    /// other's results.
    #[instrument(skip_all)]
    pub async fn update(&self, block: u64, store_events: bool) -> Result<()> {
        let _update_guard = self.update_lock.lock().await;
        self.assert_update_lock_held();
        let start = Instant::now();

        let _timer = observe::metrics::metrics()
            .on_auction_overhead_start("autopilot", "update_solvabe_orders");

        let db_solvable_orders = self.get_solvable_orders().await?;
        tracing::trace!("fetched solvable orders from db");

        // Update calls are expected to be serialized by the update task; if that
        // changes, this snapshot of previous state can become stale.
        let (
            previous_auction,
            previous_auction_id,
            previous_auction_sequence,
            previous_delta_sequence,
            previous_indexed_state,
            mut change_bundle,
        ) = {
            let cache = self.cache.lock().await;
            let previous_auction = cache.as_ref().map(|inner| inner.auction.clone());
            let previous_auction_id = cache.as_ref().map(|inner| inner.auction_id);
            let previous_auction_sequence = cache.as_ref().map(|inner| inner.auction_sequence);
            let previous_delta_sequence = cache.as_ref().map(|inner| inner.delta_sequence);
            let previous_indexed_state =
                cache.as_ref().map(|inner| Arc::clone(&inner.indexed_state));
            let change_bundle = self.diff_inputs(
                cache.as_ref().map(|inner| &inner.solvable_orders),
                &db_solvable_orders,
            );
            (
                previous_auction,
                previous_auction_id,
                previous_auction_sequence,
                previous_delta_sequence,
                previous_indexed_state,
                change_bundle,
            )
        };
        tracing::debug!(
            added = change_bundle.order_added_candidates.len(),
            removed = change_bundle.order_removed_candidates.len(),
            updated = change_bundle.order_updated_candidates.len(),
            "computed input change bundle"
        );

        let inputs = if let Some(previous_indexed_state) = previous_indexed_state.as_ref() {
            if self.incremental_primary || self.shadow_compare_incremental {
                let fallback_db_snapshot = db_solvable_orders.clone();
                match self
                    .collect_inputs_incremental(
                        previous_indexed_state.as_ref(),
                        db_solvable_orders,
                        &mut change_bundle,
                        block,
                        false,
                    )
                    .await
                {
                    Ok(incremental_inputs) if self.shadow_compare_incremental => {
                        let full_inputs = self
                            .collect_inputs_from_db(fallback_db_snapshot, block, false)
                            .await?;

                        let incremental_auction =
                            self.project_final_auction(block, &incremental_inputs)?;
                        let full_auction = self.project_final_auction(block, &full_inputs)?;

                        let incremental_events =
                            compute_delta_events(previous_auction.as_ref(), &incremental_auction);
                        let full_events =
                            compute_delta_events(previous_auction.as_ref(), &full_auction);
                        let full_indexed_state = build_indexed_state(
                            &full_auction,
                            &full_inputs.invalid_order_uids,
                            &full_inputs.filtered_order_events,
                        );
                        let indexed_state_matches = incremental_inputs
                            .indexed_state
                            .as_ref()
                            .is_some_and(|state| state.as_ref() == &full_indexed_state);
                        if !indexed_state_matches {
                            tracing::warn!(
                                "incremental indexed state diverged from full rebuild; falling \
                                 back"
                            );
                        }
                        let surfaces_match = normalized_delta_surface(incremental_auction)
                            == normalized_delta_surface(full_auction)
                            && incremental_events == full_events
                            && indexed_state_matches;

                        if surfaces_match {
                            Metrics::get()
                                .delta_shadow_compare
                                .with_label_values(&["match"])
                                .inc();
                            if self.incremental_primary {
                                Metrics::get()
                                    .delta_incremental_primary
                                    .with_label_values(&["primary"])
                                    .inc();
                                incremental_inputs
                            } else {
                                full_inputs
                            }
                        } else {
                            Metrics::get()
                                .delta_shadow_compare
                                .with_label_values(&["mismatch"])
                                .inc();
                            if self.incremental_primary {
                                Metrics::get()
                                    .delta_incremental_primary
                                    .with_label_values(&["fallback_mismatch"])
                                    .inc();
                            }
                            full_inputs
                        }
                    }
                    Ok(incremental_inputs) => {
                        Metrics::get()
                            .delta_incremental_primary
                            .with_label_values(&["primary"])
                            .inc();
                        incremental_inputs
                    }
                    Err(err) => {
                        tracing::warn!(
                            ?err,
                            "incremental input collection failed, falling back to full rebuild"
                        );
                        Metrics::get()
                            .delta_incremental_primary
                            .with_label_values(&["fallback_error"])
                            .inc();
                        self.collect_inputs_from_db(fallback_db_snapshot, block, false)
                            .await?
                    }
                }
            } else {
                self.collect_inputs_from_db(db_solvable_orders, block, false)
                    .await?
            }
        } else {
            if self.incremental_primary {
                Metrics::get()
                    .delta_incremental_primary
                    .with_label_values(&["bootstrap"])
                    .inc();
            }
            self.collect_inputs_from_db(db_solvable_orders, block, false)
                .await?
        };
        let auction = self.project_final_auction(block, &inputs)?;
        if let Some(previous_auction) = previous_auction.as_ref() {
            change_bundle.price_changed_tokens =
                Self::compute_price_changed_tokens(previous_auction, &auction);
        }
        tracing::debug!(
            price_changed = change_bundle.price_changed_tokens.len(),
            filter_transitions = change_bundle.filter_transitions.len(),
            "computed enriched change bundle"
        );

        if store_events {
            self.store_events_by_reason(
                inputs.invalid_order_uids.clone(),
                OrderEventLabel::Invalid,
            );
            self.store_events_by_reason(
                inputs.filtered_order_events.clone(),
                OrderEventLabel::Filtered,
            );
        }

        let mut cache = self.cache.lock().await;

        let next_sequence = previous_delta_sequence.map(|value| value + 1).unwrap_or(1);
        let current_auction_id = previous_auction_id.unwrap_or_default();
        let next_auction_sequence = previous_auction_sequence
            .map(|value| value + 1)
            .unwrap_or(1);
        let mut delta_history = cache
            .as_ref()
            .map(|inner| inner.delta_history.clone())
            .unwrap_or_default();

        let mut events = if inputs.indexed_state.is_some() {
            Self::compute_delta_events_from_inputs(
                previous_auction.as_ref(),
                &auction,
                &change_bundle,
                self.shadow_compare_incremental,
            )
            .unwrap_or_else(|| compute_delta_events(previous_auction.as_ref(), &auction))
        } else {
            compute_delta_events(previous_auction.as_ref(), &auction)
        };
        let (auction_for_cache, projection_mismatch) =
            self.apply_incremental_changes(previous_auction.as_ref(), auction.clone(), &events);
        if projection_mismatch {
            events = compute_delta_events(previous_auction.as_ref(), &auction);
        }
        let envelope = DeltaEnvelope {
            auction_id: current_auction_id,
            auction_sequence: next_auction_sequence,
            from_sequence: next_sequence.saturating_sub(1),
            to_sequence: next_sequence,
            published_at: chrono::Utc::now(),
            published_at_instant: Instant::now(),
            events,
        };
        delta_history.push_back(envelope.clone());
        let max_age = chrono::Duration::from_std(delta_history_max_age())
            .unwrap_or_else(|_| chrono::Duration::seconds(60));
        prune_delta_history(&mut delta_history, max_age);

        let indexed_state = if projection_mismatch {
            Arc::new(build_indexed_state(
                &auction_for_cache,
                &inputs.invalid_order_uids,
                &inputs.filtered_order_events,
            ))
        } else {
            inputs.indexed_state.unwrap_or_else(|| {
                Arc::new(build_indexed_state(
                    &auction_for_cache,
                    &inputs.invalid_order_uids,
                    &inputs.filtered_order_events,
                ))
            })
        };

        *cache = Some(Inner {
            auction: auction_for_cache,
            solvable_orders: inputs.db_solvable_orders,
            auction_id: current_auction_id,
            auction_sequence: next_auction_sequence,
            delta_sequence: next_sequence,
            delta_history,
            indexed_state,
        });
        // Replay history is updated under the cache lock; subscribers build replay
        // while holding the same lock, so sending while locked keeps replay +
        // live ordering consistent.
        let _ = self.delta_sender.send(envelope);

        tracing::debug!(%block, "updated current auction cache");
        Metrics::get()
            .auction_update_total_time
            .observe(start.elapsed().as_secs_f64());
        Ok(())
    }

    async fn collect_inputs_from_db(
        &self,
        db_solvable_orders: boundary::SolvableOrders,
        block: u64,
        store_events: bool,
    ) -> Result<CollectedAuctionInputs> {
        let orders: Vec<&Order> = db_solvable_orders
            .orders
            .values()
            .map(|order| order.as_ref())
            .collect();

        let mut invalid_order_uids = HashMap::new();
        let mut filtered_order_events: Vec<(OrderUid, OrderFilterReason)> = Vec::new();

        let balance_filter_exempt_orders: HashSet<_> = orders
            .iter()
            .filter(|order| {
                self.wrapper_cache.has_wrappers(
                    &order.data.app_data,
                    order.metadata.full_app_data.as_deref(),
                )
            })
            .map(|order| order.metadata.uid)
            .collect();

        let (balances, orders, cow_amms, in_flight) = {
            let queries = orders
                .iter()
                .map(|order| Query::from_order(order))
                .collect::<Vec<_>>();
            tokio::join!(
                self.fetch_balances(queries),
                self.filter_invalid_orders(orders, &mut invalid_order_uids),
                self.timed_future("cow_amm_registry", self.cow_amm_registry.amms()),
                self.fetch_in_flight_orders(block),
            )
        };

        let (orders, removed) = filter_out_in_flight_orders(orders, &in_flight);
        Metrics::track_filtered_orders(InFlight, &removed);
        filtered_order_events.extend(removed.into_iter().map(|uid| (uid, InFlight)));
        invalid_order_uids.retain(|uid, _| !in_flight.contains(uid));

        let orders = if self.disable_order_balance_filter {
            orders
        } else {
            let (orders, removed) = orders_with_balance(
                orders,
                &balances,
                self.settlement_contract,
                &balance_filter_exempt_orders,
            );
            Metrics::track_filtered_orders(InsufficientBalance, &removed);
            invalid_order_uids.extend(removed.into_iter().map(|uid| (uid, InsufficientBalance)));

            let (orders, removed) = filter_dust_orders(orders, &balances);
            Metrics::track_filtered_orders(DustOrder, &removed);
            filtered_order_events.extend(removed.into_iter().map(|uid| (uid, DustOrder)));

            orders
        };

        let cow_amm_tokens = cow_amms
            .iter()
            .flat_map(|cow_amm| cow_amm.traded_tokens().iter().copied())
            .collect::<Vec<_>>();

        // create auction
        let (orders, removed, mut prices) = self
            .timed_future(
                "get_orders_with_native_prices",
                get_orders_with_native_prices(
                    orders,
                    &self.native_price_estimator,
                    cow_amm_tokens,
                    self.native_price_timeout,
                ),
            )
            .await;
        tracing::trace!("fetched native prices for solvable orders");
        // Add WETH price if it's not already there to support ETH wrap when required.
        if let Entry::Vacant(entry) = prices.entry(self.weth) {
            let weth_price = self
                .timed_future(
                    "weth_price_fetch",
                    self.native_price_estimator
                        .estimate_native_price(self.weth, Default::default()),
                )
                .await
                .expect("weth price fetching can never fail");
            let weth_price = to_normalized_price(weth_price)
                .expect("weth price can never be outside of U256 range");
            entry.insert(weth_price);
        }

        Metrics::track_filtered_orders(MissingNativePrice, &removed);
        filtered_order_events.extend(removed.into_iter().map(|uid| (uid, MissingNativePrice)));
        Metrics::track_orders_in_final_auction(&orders);

        if store_events {
            self.store_events_by_reason(invalid_order_uids.clone(), OrderEventLabel::Invalid);
            self.store_events_by_reason(filtered_order_events.clone(), OrderEventLabel::Filtered);
        }

        let in_flight_owners: HashSet<_> = in_flight
            .iter()
            .map(|uid| domain::OrderUid(uid.0).owner())
            .collect();
        let surplus_capturing_jit_order_owners: Vec<_> = cow_amms
            .iter()
            .filter(|cow_amm| {
                // Orders rebalancing cow amms revert when the cow amm does not have exactly the
                // state the order was crafted for so having multiple orders in-flight for the
                // same cow amm is an issue. Additionally an amm can be rebalanced in many
                // different ways which would all result in different order UIDs so filtering
                // based on that is not sufficient. That's way we check if there is any order
                // in-flight for that amm based on the owner of the order (i.e. the cow amm) and
                // then discard that amm altogether for that auction.
                if in_flight_owners.contains(cow_amm.address()) {
                    return false;
                }
                cow_amm.traded_tokens().iter().all(|token| {
                    let price_exists = prices.contains_key(token);
                    if !price_exists {
                        tracing::debug!(
                            cow_amm = ?cow_amm.address(),
                            ?token,
                            "omitted from auction due to missing prices"
                        );
                    }
                    price_exists
                })
            })
            .map(|cow_amm| *cow_amm.address())
            .collect();

        let final_orders = tracing::info_span!("assemble_orders").in_scope(|| {
            orders
                .into_iter()
                .filter_map(|order| {
                    let uid = domain::OrderUid(order.metadata.uid.0);
                    let quote = db_solvable_orders
                        .quotes
                        .get(&uid)
                        .map(|quote| quote.as_ref().clone());
                    Some(self.protocol_fees.apply(
                        order,
                        quote,
                        &surplus_capturing_jit_order_owners,
                    ))
                })
                .collect::<Vec<_>>()
        });

        Ok(CollectedAuctionInputs {
            db_solvable_orders,
            invalid_order_uids,
            filtered_order_events,
            final_orders,
            prices,
            surplus_capturing_jit_order_owners,
            indexed_state: None,
        })
    }

    async fn collect_inputs_incremental(
        &self,
        previous_indexed_state: &IndexedAuctionState,
        db_solvable_orders: boundary::SolvableOrders,
        change_bundle: &mut ChangeBundle,
        block: u64,
        store_events: bool,
    ) -> Result<CollectedAuctionInputs> {
        let mut indexed_state = previous_indexed_state.clone();
        let mut filtered_order_events: Vec<(OrderUid, OrderFilterReason)> = Vec::new();
        let mut invalid_order_uids: HashMap<OrderUid, OrderFilterReason> = HashMap::new();
        let mut filter_transitions: Vec<FilterTransition> = Vec::new();

        let mut register_transition =
            |uid: OrderUid, reason: OrderFilterReason, was_filtered: bool, is_filtered: bool| {
                if was_filtered != is_filtered {
                    filter_transitions.push(FilterTransition {
                        uid: domain::OrderUid(uid.0),
                        reason,
                        is_filtered,
                    });
                }
            };

        for uid in &change_bundle.order_removed_candidates {
            let model_uid = OrderUid(uid.0);
            indexed_state.current_orders_by_uid.remove(uid);
            let was_invalid = indexed_state.filtered_invalid.remove(&model_uid);
            let was_in_flight = indexed_state.filtered_in_flight.remove(&model_uid);
            let was_no_balance = indexed_state.filtered_no_balance.remove(&model_uid);
            let was_no_price = indexed_state.filtered_no_price.remove(&model_uid);

            register_transition(model_uid, InvalidSignature, was_invalid, false);
            register_transition(model_uid, InFlight, was_in_flight, false);
            register_transition(model_uid, InsufficientBalance, was_no_balance, false);
            register_transition(model_uid, MissingNativePrice, was_no_price, false);
        }

        let in_flight = self.fetch_in_flight_orders(block).await;

        let mut impacted_uids = change_bundle
            .order_updated_candidates
            .iter()
            .chain(change_bundle.quote_updated_candidates.iter())
            .chain(change_bundle.order_added_candidates.iter())
            .copied()
            .collect::<HashSet<_>>();

        impacted_uids.extend(
            indexed_state
                .filtered_no_balance
                .iter()
                .map(|uid| domain::OrderUid(uid.0)),
        );
        impacted_uids.extend(
            indexed_state
                .filtered_no_price
                .iter()
                .map(|uid| domain::OrderUid(uid.0)),
        );

        impacted_uids.extend(
            in_flight
                .iter()
                .filter(|uid| {
                    indexed_state
                        .current_orders_by_uid
                        .contains_key(&domain::OrderUid(uid.0))
                })
                .map(|uid| domain::OrderUid(uid.0)),
        );
        impacted_uids.extend(
            indexed_state
                .filtered_in_flight
                .iter()
                .filter(|uid| !in_flight.contains(uid))
                .map(|uid| domain::OrderUid(uid.0)),
        );

        let queries = impacted_uids
            .iter()
            .filter_map(|uid| {
                db_solvable_orders
                    .orders
                    .get(uid)
                    .map(|order| Query::from_order(order.as_ref()))
            })
            .collect::<Vec<_>>();

        let cow_amms = self
            .timed_future("cow_amm_registry", self.cow_amm_registry.amms())
            .await;
        let balances = self.fetch_balances(queries).await;

        let mut prices: BTreeMap<Address, U256> = previous_indexed_state
            .current_prices_by_token
            .iter()
            .map(|(token, price)| (*token, price.get().0.into()))
            .collect();

        let mut impacted_tokens = impacted_uids
            .iter()
            .filter_map(|uid| db_solvable_orders.orders.get(uid))
            .flat_map(|order| [order.data.sell_token, order.data.buy_token])
            .collect::<HashSet<_>>();
        impacted_tokens.extend(cow_amms.iter().flat_map(|cow_amm| {
            cow_amm
                .traded_tokens()
                .iter()
                .copied()
                .filter(|token| !prices.contains_key(token))
        }));
        if impacted_tokens.is_empty() {
            impacted_tokens.extend(prices.keys().copied());
        }

        let fetched_impacted_prices = self
            .timed_future(
                "get_orders_with_native_prices",
                get_native_prices(
                    impacted_tokens.clone(),
                    &self.native_price_estimator,
                    self.native_price_timeout,
                ),
            )
            .await;

        let mut price_changed_tokens = Vec::new();
        for token in impacted_tokens {
            let previous = prices.get(&token).copied();
            let next = fetched_impacted_prices.get(&token).copied();
            match next {
                Some(price) => {
                    prices.insert(token, price);
                }
                None => {
                    prices.remove(&token);
                }
            }

            if previous != next {
                price_changed_tokens.push(token);
            }
        }

        if let Entry::Vacant(entry) = prices.entry(self.weth) {
            let weth_price = self
                .timed_future(
                    "weth_price_fetch",
                    self.native_price_estimator
                        .estimate_native_price(self.weth, Default::default()),
                )
                .await
                .expect("weth price fetching can never fail");
            let weth_price = to_normalized_price(weth_price)
                .expect("weth price can never be outside of U256 range");
            entry.insert(weth_price);
        }

        let changed_price_token_set = price_changed_tokens.iter().copied().collect::<HashSet<_>>();
        if !changed_price_token_set.is_empty() {
            impacted_uids.extend(
                indexed_state
                    .current_orders_by_uid
                    .keys()
                    .filter(|uid| {
                        db_solvable_orders.orders.get(uid).is_some_and(|order| {
                            changed_price_token_set.contains(&order.data.sell_token)
                                || changed_price_token_set.contains(&order.data.buy_token)
                        })
                    })
                    .copied(),
            );
        }

        let impacted_orders = impacted_uids
            .iter()
            .filter_map(|uid| {
                db_solvable_orders
                    .orders
                    .get(uid)
                    .map(|order| order.as_ref())
            })
            .collect::<Vec<_>>();

        let balance_filter_exempt_orders: HashSet<_> = impacted_orders
            .iter()
            .filter(|order| {
                self.wrapper_cache.has_wrappers(
                    &order.data.app_data,
                    order.metadata.full_app_data.as_deref(),
                )
            })
            .map(|order| order.metadata.uid)
            .collect();

        let mut invalid_for_impacted = HashMap::new();
        let mut candidate_orders = self
            .filter_invalid_orders(impacted_orders, &mut invalid_for_impacted)
            .await;
        for uid in invalid_for_impacted.keys() {
            let was_filtered = indexed_state.filtered_invalid.contains(uid);
            indexed_state.filtered_invalid.insert(*uid);
            register_transition(*uid, InvalidSignature, was_filtered, true);
            indexed_state
                .current_orders_by_uid
                .remove(&domain::OrderUid(uid.0));
        }

        let mut in_flight_removed = Vec::new();
        candidate_orders.retain(|order| {
            if in_flight.contains(&order.metadata.uid) {
                in_flight_removed.push(order.metadata.uid);
                false
            } else {
                true
            }
        });
        for uid in in_flight_removed {
            let was_filtered = indexed_state.filtered_in_flight.contains(&uid);
            indexed_state.filtered_in_flight.insert(uid);
            register_transition(uid, InFlight, was_filtered, true);
            indexed_state
                .current_orders_by_uid
                .remove(&domain::OrderUid(uid.0));
            filtered_order_events.push((uid, InFlight));
        }

        let candidate_orders = if self.disable_order_balance_filter {
            candidate_orders
        } else {
            let (candidate_orders, removed_no_balance) = orders_with_balance(
                candidate_orders,
                &balances,
                self.settlement_contract,
                &balance_filter_exempt_orders,
            );
            for uid in removed_no_balance {
                let was_filtered = indexed_state.filtered_no_balance.contains(&uid);
                indexed_state.filtered_no_balance.insert(uid);
                register_transition(uid, InsufficientBalance, was_filtered, true);
                indexed_state
                    .current_orders_by_uid
                    .remove(&domain::OrderUid(uid.0));
                invalid_for_impacted.insert(uid, InsufficientBalance);
            }

            let (candidate_orders, removed_dust) = filter_dust_orders(candidate_orders, &balances);
            for uid in removed_dust {
                indexed_state
                    .current_orders_by_uid
                    .remove(&domain::OrderUid(uid.0));
                filtered_order_events.push((uid, DustOrder));
            }
            candidate_orders
        };

        for uid in impacted_uids {
            if let Some(order) = db_solvable_orders.orders.get(&uid) {
                let model_uid = order.metadata.uid;
                if !invalid_for_impacted.contains_key(&model_uid) {
                    let was_filtered = indexed_state.filtered_invalid.remove(&model_uid);
                    register_transition(model_uid, InvalidSignature, was_filtered, false);
                }
                if !in_flight.contains(&model_uid) {
                    let was_filtered = indexed_state.filtered_in_flight.remove(&model_uid);
                    register_transition(model_uid, InFlight, was_filtered, false);
                }
            }
        }

        let mut removed_missing_price = Vec::new();
        let mut alive_orders = candidate_orders;
        alive_orders.retain(|order| {
            let has_prices = prices.contains_key(&order.data.sell_token)
                && prices.contains_key(&order.data.buy_token);
            if !has_prices {
                removed_missing_price.push(order.metadata.uid);
            }
            has_prices
        });

        for uid in removed_missing_price {
            let was_filtered = indexed_state.filtered_no_price.contains(&uid);
            indexed_state.filtered_no_price.insert(uid);
            register_transition(uid, MissingNativePrice, was_filtered, true);
            indexed_state
                .current_orders_by_uid
                .remove(&domain::OrderUid(uid.0));
            filtered_order_events.push((uid, MissingNativePrice));
        }

        let in_flight_owners: HashSet<_> = in_flight
            .iter()
            .map(|uid| domain::OrderUid(uid.0).owner())
            .collect();
        let surplus_capturing_jit_order_owners: Vec<_> = cow_amms
            .iter()
            .filter(|cow_amm| {
                if in_flight_owners.contains(cow_amm.address()) {
                    return false;
                }
                cow_amm
                    .traded_tokens()
                    .iter()
                    .all(|token| prices.contains_key(token))
            })
            .map(|cow_amm| *cow_amm.address())
            .collect();

        for order in alive_orders {
            let uid = domain::OrderUid(order.metadata.uid.0);
            let quote = db_solvable_orders
                .quotes
                .get(&uid)
                .map(|quote| quote.as_ref().clone());
            let domain_order =
                self.protocol_fees
                    .apply(order, quote, &surplus_capturing_jit_order_owners);
            indexed_state
                .current_orders_by_uid
                .insert(uid, domain_order);
            let was_no_price = indexed_state.filtered_no_price.remove(&order.metadata.uid);
            register_transition(order.metadata.uid, MissingNativePrice, was_no_price, false);
            let was_no_balance = indexed_state
                .filtered_no_balance
                .remove(&order.metadata.uid);
            register_transition(
                order.metadata.uid,
                InsufficientBalance,
                was_no_balance,
                false,
            );
        }

        invalid_order_uids.extend(invalid_for_impacted);
        if store_events {
            self.store_events_by_reason(invalid_order_uids.clone(), OrderEventLabel::Invalid);
            self.store_events_by_reason(filtered_order_events.clone(), OrderEventLabel::Filtered);
        }

        let mut final_orders = indexed_state
            .current_orders_by_uid
            .values()
            .cloned()
            .collect::<Vec<_>>();
        final_orders.sort_by_key(|order| order.uid.0);

        let entered_filters = filter_transitions
            .iter()
            .filter(|transition| transition.is_filtered)
            .count();
        let exited_filters = filter_transitions
            .iter()
            .filter(|transition| !transition.is_filtered)
            .count();
        let in_flight_transitions = filter_transitions
            .iter()
            .filter(|transition| transition.reason == InFlight)
            .count();
        let transition_uid_checksum: u64 = filter_transitions
            .iter()
            .map(|transition| u64::from(transition.uid.0[0]))
            .sum();
        tracing::debug!(
            entered_filters,
            exited_filters,
            in_flight_transitions,
            transition_uid_checksum,
            "computed incremental filter transitions"
        );

        #[cfg(debug_assertions)]
        {
            let filtered = indexed_state
                .filtered_invalid
                .iter()
                .chain(indexed_state.filtered_in_flight.iter())
                .chain(indexed_state.filtered_no_balance.iter())
                .chain(indexed_state.filtered_no_price.iter())
                .collect::<HashSet<_>>();
            debug_assert!(
                filtered.iter().all(|uid| !indexed_state
                    .current_orders_by_uid
                    .contains_key(&domain::OrderUid(uid.0))),
                "indexed_state contains filtered orders in current_orders_by_uid"
            );
        }

        change_bundle.price_changed_tokens = price_changed_tokens;
        change_bundle.filter_transitions = filter_transitions;

        Ok(CollectedAuctionInputs {
            db_solvable_orders,
            invalid_order_uids,
            filtered_order_events,
            final_orders,
            prices,
            surplus_capturing_jit_order_owners,
            indexed_state: Some(Arc::new(indexed_state)),
        })
    }

    fn project_final_auction(
        &self,
        block: u64,
        inputs: &CollectedAuctionInputs,
    ) -> Result<domain::RawAuctionData> {
        Ok(domain::RawAuctionData {
            block,
            orders: inputs.final_orders.clone(),
            prices: inputs
                .prices
                .iter()
                .map(|(token, value)| {
                    Price::try_new((*value).into()).map(|price| ((*token).into(), price))
                })
                .collect::<Result<_, _>>()?,
            surplus_capturing_jit_order_owners: inputs.surplus_capturing_jit_order_owners.clone(),
        })
    }

    fn diff_inputs(
        &self,
        previous_solvable_orders: Option<&boundary::SolvableOrders>,
        current_solvable_orders: &boundary::SolvableOrders,
    ) -> ChangeBundle {
        diff_solvable_order_inputs(previous_solvable_orders, current_solvable_orders)
    }

    fn compute_price_changed_tokens(
        previous: &domain::RawAuctionData,
        current: &domain::RawAuctionData,
    ) -> Vec<Address> {
        let mut tokens: HashSet<Address> = HashSet::new();
        tokens.extend(previous.prices.keys().map(|token| **token));
        tokens.extend(current.prices.keys().map(|token| **token));

        let mut changed = Vec::new();
        for token in tokens {
            let previous_price = previous.prices.get(&token.into());
            let current_price = current.prices.get(&token.into());
            if previous_price != current_price {
                changed.push(token);
            }
        }

        changed
    }

    fn apply_incremental_changes(
        &self,
        previous_auction: Option<&domain::RawAuctionData>,
        auction: domain::RawAuctionData,
        events: &[DeltaEvent],
    ) -> (domain::RawAuctionData, bool) {
        if let Some(previous_auction) = previous_auction {
            let reconstructed = apply_delta_events_to_auction(previous_auction.clone(), events);
            let reconstructed_surface = normalized_delta_surface(reconstructed.clone());
            let auction_surface = normalized_delta_surface(auction.clone());

            #[cfg(debug_assertions)]
            debug_assert_eq!(
                reconstructed_surface, auction_surface,
                "incremental projection mismatch; delta logic bug"
            );

            if reconstructed_surface == auction_surface {
                return (with_non_delta_fields(reconstructed, &auction), false);
            }
            Metrics::get()
                .delta_incremental_projection_mismatch_total
                .inc();
            return (auction, true);
        }
        (auction, false)
    }

    fn compute_delta_events_from_inputs(
        previous: Option<&domain::RawAuctionData>,
        current: &domain::RawAuctionData,
        change_bundle: &ChangeBundle,
        shadow_compare_incremental: bool,
    ) -> Option<Vec<DeltaEvent>> {
        let previous = previous?;
        let current_orders_by_uid = current
            .orders
            .iter()
            .map(|order| (order.uid, order.clone()))
            .collect::<HashMap<_, _>>();
        let mut current_orders = current_orders_by_uid.clone();
        let previous_orders = previous
            .orders
            .iter()
            .map(|order| (order.uid, order))
            .collect::<HashMap<_, _>>();
        let mut emitted = HashSet::new();
        let mut events = Vec::new();

        let mut added = change_bundle.order_added_candidates.clone();
        added.sort_by(|a, b| a.0.cmp(&b.0));
        added.dedup();
        for uid in added {
            if let Some(order) = current_orders.remove(&uid) {
                if emitted.insert(uid) {
                    events.push(DeltaEvent::OrderAdded(order));
                }
            }
        }

        let mut updated = change_bundle.order_updated_candidates.clone();
        updated.sort_by(|a, b| a.0.cmp(&b.0));
        updated.dedup();
        for uid in updated {
            if let Some(order) = current_orders.remove(&uid) {
                if previous_orders
                    .get(&uid)
                    .map(|previous| !solver_visible_order_eq(previous, &order))
                    .unwrap_or(true)
                {
                    if emitted.insert(uid) {
                        events.push(DeltaEvent::OrderUpdated(order));
                    }
                }
            }
        }

        let mut quote_updated = change_bundle.quote_updated_candidates.clone();
        quote_updated.sort_by(|a, b| a.0.cmp(&b.0));
        quote_updated.dedup();
        for uid in quote_updated {
            if let Some(order) = current_orders.remove(&uid) {
                if previous_orders
                    .get(&uid)
                    .map(|previous| !solver_visible_order_eq(previous, &order))
                    .unwrap_or(true)
                {
                    if emitted.insert(uid) {
                        events.push(DeltaEvent::OrderUpdated(order));
                    }
                }
            }
        }

        let mut removed = change_bundle.order_removed_candidates.clone();
        removed.sort_by(|a, b| a.0.cmp(&b.0));
        removed.dedup();
        for uid in removed {
            if emitted.insert(uid) {
                events.push(DeltaEvent::OrderRemoved(uid));
            }
        }

        let mut transitions = change_bundle.filter_transitions.clone();
        transitions.sort_by(|a, b| a.uid.0.cmp(&b.uid.0));
        for transition in transitions {
            if emitted.contains(&transition.uid) {
                continue;
            }
            if transition.is_filtered {
                if previous_orders.contains_key(&transition.uid) {
                    emitted.insert(transition.uid);
                    events.push(DeltaEvent::OrderRemoved(transition.uid));
                }
            } else if let Some(order) = current_orders_by_uid.get(&transition.uid) {
                emitted.insert(transition.uid);
                events.push(DeltaEvent::OrderAdded(order.clone()));
            } else {
                Metrics::get().delta_incremental_event_mismatch_total.inc();
                Metrics::get()
                    .delta_filter_transition_missing_order_total
                    .inc();
                tracing::warn!(uid = ?transition.uid, "missing order for filter transition");
                return None;
            }
        }

        if !change_bundle.price_changed_tokens.is_empty() {
            let mut changed_tokens = change_bundle.price_changed_tokens.clone();
            changed_tokens.sort_by(|a, b| a.as_slice().cmp(b.as_slice()));
            changed_tokens.dedup_by(|a, b| a.as_slice() == b.as_slice());
            for token in changed_tokens {
                let previous_price = previous.prices.get(&token.into());
                let current_price = current.prices.get(&token.into());
                if previous_price != current_price {
                    events.push(DeltaEvent::PriceChanged {
                        token,
                        price: current_price.copied(),
                    });
                }
            }
        }

        Metrics::get().delta_incremental_event_total.inc();

        if !shadow_compare_incremental {
            return Some(events);
        }

        let canonical = compute_delta_events(Some(previous), current);
        let canonical_matches = delta_events_equivalent(&canonical, &events);

        #[cfg(debug_assertions)]
        debug_assert!(
            canonical_matches || cfg!(test),
            "incremental delta mismatch; delta logic bug"
        );

        if canonical_matches {
            Some(events)
        } else {
            Metrics::get().delta_canonical_fallback_total.inc();
            Metrics::get().delta_incremental_event_mismatch_total.inc();
            tracing::warn!(
                canonical_events = canonical.len(),
                staged_events = events.len(),
                "incremental change bundle diverged from canonical delta; using canonical events"
            );
            Some(canonical)
        }
    }

    async fn fetch_in_flight_orders(&self, block: u64) -> HashSet<OrderUid> {
        self.persistence
            .fetch_in_flight_orders(block)
            .await
            .inspect_err(|err| tracing::warn!(?err, "failed to fetch in-flight orders"))
            .unwrap_or_default()
            .into_iter()
            .map(|uid| OrderUid(uid.0))
            .collect()
    }

    async fn fetch_balances(&self, queries: Vec<Query>) -> HashMap<Query, U256> {
        let fetched_balances = self
            .timed_future(
                "balance_filtering",
                self.balance_fetcher.get_balances(&queries),
            )
            .await;
        if self.disable_order_balance_filter {
            return Default::default();
        }

        tracing::trace!("fetched balances for solvable orders");
        queries
            .into_iter()
            .zip(fetched_balances)
            .filter_map(|(query, balance)| match balance {
                Ok(balance) => Some((query, balance)),
                Err(err) => {
                    tracing::warn!(
                        owner = ?query.owner,
                        token = ?query.token,
                        source = ?query.source,
                        error = ?err,
                        "failed to get balance"
                    );
                    None
                }
            })
            .collect()
    }

    /// Returns currently solvable orders.
    async fn get_solvable_orders(&self) -> Result<SolvableOrders> {
        let min_valid_to = now_in_epoch_seconds()
            + u32::try_from(self.min_order_validity_period.as_secs())
                .context("min_order_validity_period is not u32")?;

        // only build future while holding the lock but execute outside of lock
        let lock = self.cache.lock().await;
        let fetch_orders = match &*lock {
            // Only use incremental query after cache already got initialized
            // because it's not optimized for very long durations.
            Some(cache) => {
                tracing::trace!(
                    indexed_orders = cache.indexed_state.current_orders_by_uid.len(),
                    indexed_prices = cache.indexed_state.current_prices_by_token.len(),
                    "using cached indexed state as incremental baseline"
                );
                self.persistence
                    .solvable_orders_after(
                        cache.solvable_orders.orders.clone(),
                        cache.solvable_orders.quotes.clone(),
                        cache.solvable_orders.fetched_from_db,
                        cache.solvable_orders.latest_settlement_block,
                        min_valid_to,
                    )
                    .boxed()
            }
            None => self.persistence.all_solvable_orders(min_valid_to).boxed(),
        };

        let mut orders = fetch_orders.await?;

        // Move the checkpoint slightly back in time to mitigate race conditions
        // caused by inconsistencies of stored timestamps. See #2959 for more details.
        // This will cause us to fetch orders created or cancelled in the buffer
        // period multiple times but that is a small price to pay for not missing
        // orders.
        orders.fetched_from_db -= chrono::TimeDelta::seconds(60);
        Ok(orders)
    }

    /// Executed orders filtering in parallel.
    #[instrument(skip_all)]
    async fn filter_invalid_orders<'a>(
        &self,
        mut orders: Vec<&'a Order>,
        invalid_order_uids: &mut HashMap<OrderUid, OrderFilterReason>,
    ) -> Vec<&'a Order> {
        let presignature_pending_orders = find_presignature_pending_orders(&orders);

        let unsupported_token_orders = find_unsupported_tokens(&orders, &self.deny_listed_tokens);
        let banned_user_orders = self
            .timed_future(
                "banned_user_filtering",
                find_banned_user_orders(&orders, &self.banned_users),
            )
            .await;
        tracing::trace!("filtered invalid orders");

        Metrics::track_filtered_orders(BannedUser, &banned_user_orders);
        Metrics::track_filtered_orders(InvalidSignature, &presignature_pending_orders);
        Metrics::track_filtered_orders(UnsupportedToken, &unsupported_token_orders);
        invalid_order_uids.extend(banned_user_orders.into_iter().map(|uid| (uid, BannedUser)));
        invalid_order_uids.extend(
            presignature_pending_orders
                .into_iter()
                .map(|uid| (uid, InvalidSignature)),
        );
        invalid_order_uids.extend(
            unsupported_token_orders
                .into_iter()
                .map(|uid| (uid, UnsupportedToken)),
        );

        orders.retain(|order| !invalid_order_uids.contains_key(&order.metadata.uid));
        orders
    }

    pub fn track_auction_update(&self, result: &str) {
        Metrics::get()
            .auction_update
            .with_label_values(&[result])
            .inc();
    }

    /// Runs the future and collects runtime metrics.
    async fn timed_future<T>(&self, label: &str, fut: impl Future<Output = T>) -> T {
        let _timer = Metrics::get()
            .auction_update_stage_time
            .with_label_values(&[label])
            .start_timer();
        fut.await
    }

    fn store_events_by_reason(
        &self,
        orders: impl IntoIterator<Item = (OrderUid, OrderFilterReason)>,
        label: OrderEventLabel,
    ) {
        let mut by_reason: HashMap<OrderFilterReason, Vec<OrderUid>> = HashMap::new();
        for (uid, reason) in orders {
            by_reason.entry(reason).or_default().push(uid);
        }
        for (reason, uids) in by_reason {
            self.persistence.store_order_events_owned(
                uids,
                |uid| domain::OrderUid(uid.0),
                label,
                Some(reason),
            );
        }
    }
}

fn prune_delta_history(delta_history: &mut VecDeque<DeltaEnvelope>, max_age: chrono::Duration) {
    let max_age = max_age.to_std().unwrap_or_else(|_| Duration::from_secs(0));
    let min_retained = delta_history_min_retained();
    loop {
        let over_count = delta_history.len() > MAX_DELTA_HISTORY;
        let over_age = delta_history
            .front()
            .is_some_and(|front| front.published_at_instant.elapsed() > max_age)
            && delta_history.len() > min_retained;

        if !(over_count || over_age) {
            break;
        }

        delta_history.pop_front();
    }
}

fn apply_auction_id_change(inner: &mut Inner, auction_id: u64) -> Option<DeltaEnvelope> {
    if inner.auction_id == auction_id {
        return None;
    }

    inner.auction_id = auction_id;
    inner.auction_sequence = 0;
    // Keep delta_sequence monotonic across auctions for replay continuity.
    let next_sequence = inner.delta_sequence.saturating_add(1);
    let envelope = DeltaEnvelope {
        auction_id,
        auction_sequence: 0,
        from_sequence: inner.delta_sequence,
        to_sequence: next_sequence,
        published_at: chrono::Utc::now(),
        published_at_instant: Instant::now(),
        events: vec![DeltaEvent::AuctionChanged {
            new_auction_id: auction_id,
        }],
    };
    inner.delta_sequence = next_sequence;
    inner.delta_history.push_back(envelope.clone());
    let max_age = chrono::Duration::from_std(delta_history_max_age())
        .unwrap_or_else(|_| chrono::Duration::seconds(60));
    prune_delta_history(&mut inner.delta_history, max_age);
    Some(envelope)
}

fn delta_history_min_retained() -> usize {
    std::env::var("AUTOPILOT_DELTA_SYNC_HISTORY_MIN_RETAINED")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &usize| *value > 0)
        .unwrap_or(MIN_DELTA_HISTORY_RETAINED)
}

fn delta_history_max_age() -> Duration {
    std::env::var("AUTOPILOT_DELTA_SYNC_HISTORY_MAX_AGE_SECS")
        .ok()
        .and_then(|value| value.parse().ok())
        .filter(|value: &u64| *value > 0)
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_DELTA_HISTORY_MAX_AGE)
}

fn delta_events_equivalent(canonical: &[DeltaEvent], staged: &[DeltaEvent]) -> bool {
    // Ordering is normalized by uid then event rank; this treats different event
    // orderings for the same uid as equivalent, even though such mixes should
    // never occur in valid deltas.
    let mut canonical_sorted = canonical.to_vec();
    canonical_sorted.sort_by(delta_event_cmp);
    let mut staged_sorted = staged.to_vec();
    staged_sorted.sort_by(delta_event_cmp);
    canonical_sorted == staged_sorted
}

fn delta_event_cmp(lhs: &DeltaEvent, rhs: &DeltaEvent) -> Ordering {
    let lhs_group = delta_event_rank(lhs);
    let rhs_group = delta_event_rank(rhs);
    match lhs_group.cmp(&rhs_group) {
        Ordering::Equal => {}
        other => return other,
    }

    let lhs_key = order_event_key(lhs);
    let rhs_key = order_event_key(rhs);
    match (lhs_key, rhs_key) {
        (Some((uid_a, rank_a)), Some((uid_b, rank_b))) => {
            uid_a.cmp(&uid_b).then(rank_a.cmp(&rank_b))
        }
        (None, None) => match (lhs, rhs) {
            (
                DeltaEvent::AuctionChanged {
                    new_auction_id: lhs,
                },
                DeltaEvent::AuctionChanged {
                    new_auction_id: rhs,
                },
            ) => lhs.cmp(rhs),
            (
                DeltaEvent::PriceChanged { token: a, .. },
                DeltaEvent::PriceChanged { token: b, .. },
            ) => a.as_slice().cmp(b.as_slice()),
            _ => Ordering::Equal,
        },
        _ => Ordering::Equal,
    }
}

fn order_event_key(event: &DeltaEvent) -> Option<([u8; 56], u8)> {
    match event {
        DeltaEvent::OrderAdded(order) => Some((order.uid.0, 0)),
        DeltaEvent::OrderUpdated(order) => Some((order.uid.0, 1)),
        DeltaEvent::OrderRemoved(uid) => Some((uid.0, 2)),
        DeltaEvent::AuctionChanged { .. } | DeltaEvent::PriceChanged { .. } => None,
    }
}

fn delta_event_rank(event: &DeltaEvent) -> u8 {
    match event {
        DeltaEvent::AuctionChanged { .. } => 0,
        DeltaEvent::OrderAdded(_) => 1,
        DeltaEvent::OrderUpdated(_) => 2,
        DeltaEvent::OrderRemoved(_) => 3,
        DeltaEvent::PriceChanged { .. } => 4,
    }
}

fn checksum_order_uids(orders: &[domain::Order]) -> String {
    let mut uids = orders.iter().map(|order| order.uid).collect::<Vec<_>>();
    uids.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for uid in uids {
        hasher.update(uid.0);
    }
    format!("0x{}", const_hex::encode(hasher.finalize()))
}

fn checksum_prices(prices: &domain::auction::Prices) -> String {
    let mut entries = prices.iter().collect::<Vec<_>>();
    entries.sort_by(|(lhs, _), (rhs, _)| lhs.as_slice().cmp(rhs.as_slice()));

    let mut hasher = Sha256::new();
    for (token, price) in entries {
        hasher.update(token.as_slice());
        hasher.update(price.get().0.to_string().as_bytes());
    }
    format!("0x{}", const_hex::encode(hasher.finalize()))
}

fn compute_delta_events(
    previous: Option<&domain::RawAuctionData>,
    current: &domain::RawAuctionData,
) -> Vec<DeltaEvent> {
    let Some(previous) = previous else {
        let order_events = current.orders.iter().cloned().map(DeltaEvent::OrderAdded);
        let price_events = current.prices.keys().map(|token| {
            let token = Address::from(*token);
            DeltaEvent::PriceChanged {
                token,
                price: current.prices.get(&token.into()).copied(),
            }
        });
        return order_events.chain(price_events).collect();
    };

    let previous_orders = previous
        .orders
        .iter()
        .map(|order| (order.uid, order))
        .collect::<HashMap<_, _>>();
    let current_orders = current
        .orders
        .iter()
        .map(|order| (order.uid, order))
        .collect::<HashMap<_, _>>();

    let mut events = Vec::new();

    let mut current_uids = current_orders.keys().copied().collect::<Vec<_>>();
    current_uids.sort_by(|a, b| a.0.cmp(&b.0));
    for uid in current_uids {
        let current_order = current_orders
            .get(&uid)
            .expect("uid from keys must exist in map");
        match previous_orders.get(&uid) {
            None => events.push(DeltaEvent::OrderAdded((*current_order).clone())),
            Some(previous_order) if !solver_visible_order_eq(previous_order, current_order) => {
                events.push(DeltaEvent::OrderUpdated((*current_order).clone()));
            }
            Some(_) => {}
        }
    }

    let mut removed_uids = previous_orders
        .keys()
        .filter(|uid| !current_orders.contains_key(uid))
        .copied()
        .collect::<Vec<_>>();
    removed_uids.sort_by(|a, b| a.0.cmp(&b.0));
    for uid in removed_uids {
        events.push(DeltaEvent::OrderRemoved(uid));
    }

    let mut price_tokens = previous
        .prices
        .keys()
        .map(|token| Address::from(*token))
        .collect::<Vec<_>>();
    price_tokens.extend(current.prices.keys().map(|token| Address::from(*token)));
    price_tokens.sort_by(|a, b| a.as_slice().cmp(b.as_slice()));
    price_tokens.dedup_by(|a, b| a.as_slice() == b.as_slice());
    for token in price_tokens {
        let previous_price = previous.prices.get(&token.into());
        let current_price = current.prices.get(&token.into());
        if previous_price != current_price {
            events.push(DeltaEvent::PriceChanged {
                token,
                price: current_price.copied(),
            });
        }
    }

    events
}

fn apply_delta_events_to_auction(
    previous: domain::RawAuctionData,
    events: &[DeltaEvent],
) -> domain::RawAuctionData {
    let mut orders: HashMap<domain::OrderUid, domain::Order> = previous
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

fn solver_visible_order_eq(lhs: &domain::Order, rhs: &domain::Order) -> bool {
    lhs.uid == rhs.uid
        && lhs.sell == rhs.sell
        && lhs.buy == rhs.buy
        && lhs.protocol_fees == rhs.protocol_fees
        && lhs.side == rhs.side
        && lhs.created == rhs.created
        && lhs.valid_to == rhs.valid_to
        && lhs.receiver == rhs.receiver
        && lhs.owner == rhs.owner
        && lhs.partially_fillable == rhs.partially_fillable
        && lhs.executed == rhs.executed
        && lhs.pre_interactions == rhs.pre_interactions
        && lhs.post_interactions == rhs.post_interactions
        && lhs.sell_token_balance == rhs.sell_token_balance
        && lhs.buy_token_balance == rhs.buy_token_balance
        && lhs.app_data == rhs.app_data
        && lhs.signature == rhs.signature
        && lhs.quote == rhs.quote
}

fn normalized_delta_surface(mut auction: domain::RawAuctionData) -> domain::RawAuctionData {
    auction.block = 0;
    auction.surplus_capturing_jit_order_owners.clear();
    auction.orders.sort_by_key(|order| order.uid.0);
    auction
}

fn with_non_delta_fields(
    mut reconstructed: domain::RawAuctionData,
    full_rebuild: &domain::RawAuctionData,
) -> domain::RawAuctionData {
    reconstructed.block = full_rebuild.block;
    reconstructed.surplus_capturing_jit_order_owners =
        full_rebuild.surplus_capturing_jit_order_owners.clone();
    reconstructed
}

fn diff_solvable_order_inputs(
    previous: Option<&boundary::SolvableOrders>,
    current: &boundary::SolvableOrders,
) -> ChangeBundle {
    // Complexity: O(|previous| + |current|) over order/quote maps.
    let Some(previous) = previous else {
        return ChangeBundle {
            order_added_candidates: current.orders.keys().copied().collect(),
            ..Default::default()
        };
    };

    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut quote_updated = Vec::new();

    for (uid, order) in &current.orders {
        let mut order_changed = false;
        match previous.orders.get(uid) {
            None => {
                added.push(*uid);
                order_changed = true;
            }
            Some(previous_order) if previous_order.as_ref() != order.as_ref() => {
                updated.push(*uid);
                order_changed = true;
            }
            Some(_) => {}
        }

        if !order_changed && previous.quotes.get(uid) != current.quotes.get(uid) {
            quote_updated.push(*uid);
        }
    }

    updated.sort_by(|a, b| a.0.cmp(&b.0));
    updated.dedup();

    quote_updated.sort_by(|a, b| a.0.cmp(&b.0));
    quote_updated.dedup();

    let removed = previous
        .orders
        .keys()
        .filter(|uid| !current.orders.contains_key(uid))
        .copied()
        .collect();

    ChangeBundle {
        order_added_candidates: added,
        order_removed_candidates: removed,
        order_updated_candidates: updated,
        quote_updated_candidates: quote_updated,
        price_changed_tokens: Vec::new(),
        filter_transitions: Vec::new(),
    }
}

fn build_indexed_state(
    auction: &domain::RawAuctionData,
    invalid_order_uids: &HashMap<OrderUid, OrderFilterReason>,
    filtered_order_events: &[(OrderUid, OrderFilterReason)],
) -> IndexedAuctionState {
    let mut state = IndexedAuctionState {
        current_orders_by_uid: auction
            .orders
            .iter()
            .cloned()
            .map(|order| (order.uid, order))
            .collect(),
        current_prices_by_token: auction
            .prices
            .iter()
            .map(|(token, price)| (Address::from(*token), *price))
            .collect(),
        ..Default::default()
    };

    for uid in invalid_order_uids.keys() {
        state.filtered_invalid.insert(*uid);
    }

    for (uid, reason) in filtered_order_events {
        match reason {
            OrderFilterReason::InFlight => {
                state.filtered_in_flight.insert(*uid);
            }
            OrderFilterReason::InsufficientBalance => {
                state.filtered_no_balance.insert(*uid);
            }
            OrderFilterReason::MissingNativePrice => {
                state.filtered_no_price.insert(*uid);
            }
            _ => {}
        }
    }

    state
}

/// Finds all orders whose owners or receivers are in the set of "banned"
/// users.
async fn find_banned_user_orders(orders: &[&Order], banned_users: &banned::Users) -> Vec<OrderUid> {
    let banned = banned_users
        .banned(
            orders
                .iter()
                .flat_map(|order| std::iter::once(order.metadata.owner).chain(order.data.receiver)),
        )
        .await;
    orders
        .iter()
        .filter_map(|order| {
            std::iter::once(order.metadata.owner)
                .chain(order.data.receiver)
                .any(|addr| banned.contains(&addr))
                .then_some(order.metadata.uid)
        })
        .collect()
}

async fn get_native_prices(
    tokens: HashSet<Address>,
    native_price_estimator: &NativePriceUpdater,
    timeout: Duration,
) -> BTreeMap<Address, alloy::primitives::U256> {
    native_price_estimator
        .update_tokens_and_fetch_prices(tokens, timeout)
        .await
        .into_iter()
        .flat_map(|(token, result)| {
            let price = to_normalized_price(result.ok()?)?;
            Some((token, price))
        })
        .collect()
}

/// Finds orders with pending presignatures. EIP-1271 signature validation is
/// skipped entirely - the driver validates signatures before settlement.
fn find_presignature_pending_orders(orders: &[&Order]) -> Vec<OrderUid> {
    orders
        .iter()
        .filter(|order| {
            matches!(
                order.metadata.status,
                model::order::OrderStatus::PresignaturePending
            )
        })
        .map(|order| order.metadata.uid)
        .collect()
}

/// Removes orders that can't possibly be settled because there isn't enough
/// balance.
#[instrument(skip_all)]
fn orders_with_balance<'a>(
    mut orders: Vec<&'a Order>,
    balances: &Balances,
    settlement_contract: Address,
    filter_bypass_orders: &HashSet<OrderUid>,
) -> (Vec<&'a Order>, Vec<OrderUid>) {
    // Prefer newer orders over older ones.
    orders.sort_by_key(|order| std::cmp::Reverse(order.metadata.creation_date));
    let mut filtered_orders = vec![];
    let keep = |order: &Order| {
        // Skip balance check for all EIP-1271 orders (they can rely on pre-interactions
        // to unlock funds) or orders with wrappers (wrappers produce the required
        // balance at settlement time).
        if matches!(order.signature, Signature::Eip1271(_))
            || filter_bypass_orders.contains(&order.metadata.uid)
        {
            return true;
        }

        if order.data.receiver.as_ref() == Some(&settlement_contract) {
            // TODO: replace with proper detection logic
            // for now we assume that all orders with the settlement contract
            // as the receiver are flashloan orders which unlock the necessary
            // funds via a pre-interaction that can't succeed in our balance
            // fetching simulation logic.
            return true;
        }

        let balance = match balances.get(&Query::from_order(order)) {
            None => return false,
            Some(balance) => *balance,
        };

        if order.data.partially_fillable && balance >= U256::ONE {
            return true;
        }

        let needed_balance = match order.data.sell_amount.checked_add(order.data.fee_amount) {
            None => return false,
            Some(balance) => balance,
        };
        balance >= needed_balance
    };

    orders.retain(|order| {
        if keep(order) {
            true
        } else {
            filtered_orders.push(order.metadata.uid);
            false
        }
    });
    (orders, filtered_orders)
}

/// Filters out dust orders i.e. partially fillable orders that, when scaled
/// have a 0 buy or sell amount.
fn filter_dust_orders<'a>(
    mut orders: Vec<&'a Order>,
    balances: &Balances,
) -> (Vec<&'a Order>, Vec<OrderUid>) {
    let mut removed = vec![];
    let keep = |order: &Order| {
        if !order.data.partially_fillable {
            return true;
        }

        let balance = if let Some(balance) = balances.get(&Query::from_order(order)) {
            *balance
        } else {
            return false;
        };

        let Ok(remaining) =
            remaining_amounts::Remaining::from_order_with_balance(&order.into(), balance)
        else {
            return false;
        };

        let (Ok(sell_amount), Ok(buy_amount)) = (
            remaining.remaining(order.data.sell_amount),
            remaining.remaining(order.data.buy_amount),
        ) else {
            return false;
        };

        !sell_amount.is_zero() && !buy_amount.is_zero()
    };

    orders.retain(|order| {
        if keep(order) {
            true
        } else {
            removed.push(order.metadata.uid);
            false
        }
    });
    (orders, removed)
}

#[instrument(skip_all)]
async fn get_orders_with_native_prices<'a>(
    orders: Vec<&'a Order>,
    native_price_estimator: &NativePriceUpdater,
    additional_tokens: impl IntoIterator<Item = Address>,
    timeout: Duration,
) -> (
    Vec<&'a Order>,
    Vec<OrderUid>,
    BTreeMap<Address, alloy::primitives::U256>,
) {
    let traded_tokens = orders
        .iter()
        .flat_map(|order| [order.data.sell_token, order.data.buy_token])
        .chain(additional_tokens)
        .collect::<HashSet<_>>();

    let prices = get_native_prices(traded_tokens, native_price_estimator, timeout).await;

    // Filter orders so that we only return orders that have prices
    let mut removed_market_orders = 0_i64;
    let mut removed_orders = vec![];
    let mut orders = orders;
    orders.retain(|order| {
        let both_prices_present = prices.contains_key(&order.data.sell_token)
            && prices.contains_key(&order.data.buy_token);
        if both_prices_present {
            true
        } else {
            removed_orders.push(order.metadata.uid);
            removed_market_orders += i64::from(order.metadata.class == OrderClass::Market);
            false
        }
    });

    Metrics::get()
        .auction_market_order_missing_price
        .set(removed_market_orders);

    (orders, removed_orders, prices)
}

fn find_unsupported_tokens(
    orders: &[&Order],
    deny_listed_tokens: &DenyListedTokens,
) -> Vec<OrderUid> {
    orders
        .iter()
        .filter_map(|order| {
            [&order.data.buy_token, &order.data.sell_token]
                .iter()
                .any(|token| deny_listed_tokens.contains(token))
                .then_some(order.metadata.uid)
        })
        .collect()
}

fn filter_out_in_flight_orders<'a>(
    mut orders: Vec<&'a Order>,
    in_flight: &HashSet<OrderUid>,
) -> (Vec<&'a Order>, Vec<OrderUid>) {
    let mut removed = vec![];
    orders.retain(|order| {
        if in_flight.contains(&order.metadata.uid) {
            removed.push(order.metadata.uid);
            false
        } else {
            true
        }
    });
    (orders, removed)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            database::{Config as DbConfig, Postgres},
            infra::Persistence,
        },
        account_balances::BalanceFetching,
        alloy::primitives::{Address, B256},
        bad_tokens::list_based::DenyListedTokens,
        eth_domain_types as eth,
        ethrpc::{
            alloy::unbuffered_provider,
            block_stream::{BlockInfo, mock_single_block},
        },
        event_indexing::block_retriever::BlockRetriever,
        futures::FutureExt,
        maplit::{btreemap, hashset},
        model::order::{OrderBuilder, OrderData, OrderMetadata, OrderUid},
        price_estimation::{
            HEALTHY_PRICE_ESTIMATION_TIME,
            PriceEstimationError,
            native::MockNativePriceEstimating,
            native_price_cache::{
                ApproximationToken,
                Cache,
                CachingNativePriceEstimator,
                NativePriceUpdater,
            },
        },
        sqlx::postgres::PgPoolOptions,
        std::collections::HashMap,
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

    impl price_estimation::native::NativePriceEstimating for StubNativePriceEstimator {
        fn estimate_native_price(
            &self,
            _token: Address,
            _timeout: Duration,
        ) -> futures::future::BoxFuture<'_, price_estimation::native::NativePriceEstimateResult>
        {
            async { Ok(1.0) }.boxed()
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
        let cow_amm_registry = cow_amm::Registry::new(block_retriever);

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

    #[tokio::test]
    async fn get_orders_with_native_prices_with_timeout() {
        let token1 = Address::repeat_byte(1);
        let token2 = Address::repeat_byte(2);
        let token3 = Address::repeat_byte(3);

        let orders = [
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token2)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token3)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
        ];

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf(move |token, _| *token == token1)
            .returning(|_, _| async { Ok(2.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token2)
            .returning(|_, _| async { Err(PriceEstimationError::NoLiquidity) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token3)
            .returning(|_, _| async { Ok(0.25) }.boxed());

        let cache = Cache::new(Duration::from_secs(10), Default::default());
        let caching_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            cache,
            3,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let native_price_estimator =
            NativePriceUpdater::new(caching_estimator, Duration::MAX, Default::default());

        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (filtered_orders, _removed, prices) = get_orders_with_native_prices(
            orders_ref,
            &native_price_estimator,
            vec![],
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(filtered_orders, [orders[1].as_ref()]);
        assert_eq!(
            prices,
            btreemap! {
                token1 => alloy::primitives::U256::from(2_000_000_000_000_000_000_u128),
                token3 => alloy::primitives::U256::from(250_000_000_000_000_000_u128),
            }
        );
    }

    #[tokio::test]
    async fn filters_orders_with_tokens_without_native_prices() {
        let token1 = Address::repeat_byte(1);
        let token2 = Address::repeat_byte(2);
        let token3 = Address::repeat_byte(3);
        let token4 = Address::repeat_byte(4);
        let token5 = Address::repeat_byte(5);

        let orders = [
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token2)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token2)
                    .with_buy_token(token3)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token3)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token2)
                    .with_buy_token(token4)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
        ];

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf(move |token, _| *token == token1)
            .returning(|_, _| async { Ok(2.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token2)
            .returning(|_, _| async { Err(PriceEstimationError::NoLiquidity) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token3)
            .returning(|_, _| async { Ok(0.25) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token4)
            .returning(|_, _| async { Ok(0.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token5)
            .returning(|_, _| async { Ok(5.) }.boxed());

        let cache = Cache::new(Duration::from_secs(10), Default::default());
        let caching_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            cache,
            1,
            Default::default(),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let native_price_estimator = NativePriceUpdater::new(
            caching_estimator,
            Duration::from_millis(5),
            Default::default(),
        );

        // We'll have no native prices in this call. But set_tokens_to_update
        // will cause the background task to fetch them in the next cycle.
        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (alive_orders, _removed_orders, prices) = get_orders_with_native_prices(
            orders_ref,
            &native_price_estimator,
            vec![token5],
            Duration::ZERO,
        )
        .await;
        assert!(alive_orders.is_empty());
        assert!(prices.is_empty());

        // Wait for native prices to get fetched by the background task.
        tokio::time::sleep(tokio::time::Duration::from_millis(30)).await;

        // Now we have all the native prices we want.
        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (alive_orders, _removed_orders, prices) = get_orders_with_native_prices(
            orders_ref,
            &native_price_estimator,
            vec![token5],
            Duration::ZERO,
        )
        .await;

        assert_eq!(alive_orders, [orders[2].as_ref()]);
        assert_eq!(
            prices,
            btreemap! {
                token1 => alloy::primitives::U256::from(2_000_000_000_000_000_000_u128),
                token3 => alloy::primitives::U256::from(250_000_000_000_000_000_u128),
                token5 => alloy::primitives::U256::from(5_000_000_000_000_000_000_u128),
            }
        );
    }

    #[tokio::test]
    async fn check_native_price_approximations() {
        let token1 = Address::repeat_byte(1);
        let token2 = Address::repeat_byte(2);
        let token3 = Address::repeat_byte(3);

        let token_approx1 = Address::repeat_byte(4);
        let token_approx2 = Address::repeat_byte(5);

        let orders = [
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token2)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token2)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token3)
                    .with_buy_amount(alloy::primitives::U256::ONE)
                    .with_sell_amount(alloy::primitives::U256::ONE)
                    .build(),
            ),
        ];

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token3)
            .returning(|_, _| async { Ok(3.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token_approx1)
            .returning(|_, _| async { Ok(40.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token, _| *token == token_approx2)
            .returning(|_, _| async { Ok(50.) }.boxed());

        let cache = Cache::new(Duration::from_secs(10), Default::default());
        let caching_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            cache,
            3,
            // Set to use native price approximations for the following tokens
            HashMap::from([
                (token1, ApproximationToken::same_decimals(token_approx1)),
                (token2, ApproximationToken::same_decimals(token_approx2)),
            ]),
            HEALTHY_PRICE_ESTIMATION_TIME,
        );
        let native_price_estimator =
            NativePriceUpdater::new(caching_estimator, Duration::MAX, Default::default());

        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (alive_orders, _removed_orders, prices) = get_orders_with_native_prices(
            orders_ref,
            &native_price_estimator,
            vec![],
            Duration::from_secs(10),
        )
        .await;
        assert!(
            alive_orders
                .iter()
                .copied()
                .eq(orders.iter().map(Arc::as_ref))
        );
        assert_eq!(
            prices,
            btreemap! {
                token1 => alloy::primitives::U256::from(40_000_000_000_000_000_000_u128),
                token2 => alloy::primitives::U256::from(50_000_000_000_000_000_000_u128),
                token3 => alloy::primitives::U256::from(3_000_000_000_000_000_000_u128),
            }
        );
    }

    #[tokio::test]
    async fn filters_banned_users() {
        let banned_users = hashset!(Address::from([0xba; 20]), Address::from([0xbb; 20]));
        let orders = [
            Address::repeat_byte(1),
            Address::repeat_byte(1),
            Address::repeat_byte(0xba),
            Address::repeat_byte(2),
            Address::repeat_byte(0xba),
            Address::repeat_byte(0xbb),
            Address::repeat_byte(3),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, owner)| {
            Arc::new(Order {
                metadata: OrderMetadata {
                    owner,
                    uid: OrderUid([i as u8; 56]),
                    ..Default::default()
                },
                data: OrderData {
                    buy_amount: alloy::primitives::U256::ONE,
                    sell_amount: alloy::primitives::U256::ONE,
                    ..Default::default()
                },
                ..Default::default()
            })
        })
        .collect::<Vec<_>>();

        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let banned_user_orders = find_banned_user_orders(
            &orders_ref,
            &order_validation::banned::Users::from_set(banned_users),
        )
        .await;
        assert_eq!(
            banned_user_orders,
            [OrderUid([2; 56]), OrderUid([4; 56]), OrderUid([5; 56])],
        );
    }

    #[test]
    fn finds_presignature_pending_orders() {
        let presign_uid = OrderUid::from_parts(B256::repeat_byte(1), Address::repeat_byte(11), 1);
        let orders = [
            // PresignaturePending order - should be found
            Arc::new(Order {
                metadata: OrderMetadata {
                    uid: presign_uid,
                    status: model::order::OrderStatus::PresignaturePending,
                    ..Default::default()
                },
                ..Default::default()
            }),
            // EIP-1271 order - not PresignaturePending
            Arc::new(Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(B256::repeat_byte(2), Address::repeat_byte(22), 2),
                    ..Default::default()
                },
                signature: Signature::Eip1271(vec![2, 2]),
                ..Default::default()
            }),
            // Regular order - not PresignaturePending
            Arc::new(Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(B256::repeat_byte(3), Address::repeat_byte(33), 3),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ];

        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let pending_orders = find_presignature_pending_orders(&orders_ref);
        assert_eq!(pending_orders, vec![presign_uid]);
    }

    #[test]
    fn filter_unsupported_tokens_() {
        let token0 = Address::with_last_byte(0);
        let token1 = Address::with_last_byte(1);
        let token2 = Address::with_last_byte(2);
        let deny_listed_tokens = DenyListedTokens::new(vec![token0]);
        let orders = [
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token0)
                    .with_buy_token(token1)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token1)
                    .with_buy_token(token2)
                    .build(),
            ),
            Arc::new(
                OrderBuilder::default()
                    .with_sell_token(token0)
                    .with_buy_token(token2)
                    .build(),
            ),
        ];
        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let unsupported_tokens_orders = find_unsupported_tokens(&orders_ref, &deny_listed_tokens);
        assert_eq!(
            unsupported_tokens_orders,
            [orders[0].metadata.uid, orders[2].metadata.uid]
        );
    }

    #[test]
    fn orders_with_balance_() {
        let settlement_contract = Address::repeat_byte(1);
        let orders = [
            // enough balance for sell and fee
            Arc::new(Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(2),
                    sell_amount: alloy::primitives::U256::ONE,
                    fee_amount: alloy::primitives::U256::ONE,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }),
            // missing fee balance
            Arc::new(Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(3),
                    sell_amount: alloy::primitives::U256::ONE,
                    fee_amount: alloy::primitives::U256::ONE,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            }),
            // at least 1 partially fillable balance
            Arc::new(Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(4),
                    sell_amount: alloy::primitives::U256::from(2),
                    fee_amount: alloy::primitives::U256::ZERO,
                    partially_fillable: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            // 0 partially fillable balance
            Arc::new(Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(5),
                    sell_amount: alloy::primitives::U256::from(2),
                    fee_amount: alloy::primitives::U256::ZERO,
                    partially_fillable: true,
                    ..Default::default()
                },
                ..Default::default()
            }),
            // considered flashloan order because of special receiver
            Arc::new(Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(6),
                    sell_amount: alloy::primitives::U256::from(200),
                    fee_amount: alloy::primitives::U256::ZERO,
                    partially_fillable: true,
                    receiver: Some(settlement_contract),
                    ..Default::default()
                },
                ..Default::default()
            }),
        ];
        let balances = [
            (Query::from_order(&orders[0]), U256::from(2)),
            (Query::from_order(&orders[1]), U256::from(1)),
            (Query::from_order(&orders[2]), U256::from(1)),
            (Query::from_order(&orders[3]), U256::from(0)),
            (Query::from_order(&orders[4]), U256::from(0)),
        ]
        .into_iter()
        .collect();
        let expected = &[0, 2, 4];

        let no_bypass: HashSet<OrderUid> = HashSet::new();
        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (alive_orders, _removed_orders) =
            orders_with_balance(orders_ref, &balances, settlement_contract, &no_bypass);
        assert_eq!(alive_orders.len(), expected.len());
        for index in expected {
            let found = alive_orders.iter().any(|o| o.data == orders[*index].data);
            assert!(found, "{}", index);
        }
    }

    #[test]
    fn eip1271_and_wrapper_orders_skip_balance_filtering() {
        let settlement_contract = Address::repeat_byte(1);

        // EIP-1271 order (should skip balance check)
        let eip1271_order = Arc::new(Order {
            data: OrderData {
                sell_token: Address::with_last_byte(7),
                sell_amount: alloy::primitives::U256::from(10),
                fee_amount: alloy::primitives::U256::from(5),
                partially_fillable: false,
                ..Default::default()
            },
            signature: Signature::Eip1271(vec![1, 2, 3]),
            metadata: OrderMetadata {
                uid: OrderUid::from_parts(B256::repeat_byte(6), Address::repeat_byte(66), 6),
                ..Default::default()
            },
            ..Default::default()
        });

        // Order with wrappers in bypass set (should skip balance check)
        let wrapper_order_uid =
            OrderUid::from_parts(B256::repeat_byte(7), Address::repeat_byte(77), 7);
        let wrapper_order = Arc::new(Order {
            data: OrderData {
                sell_token: Address::with_last_byte(8),
                sell_amount: alloy::primitives::U256::from(10),
                fee_amount: alloy::primitives::U256::from(5),
                partially_fillable: false,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: wrapper_order_uid,
                ..Default::default()
            },
            ..Default::default()
        });

        // Regular ECDSA order without wrappers (should be filtered)
        let regular_order = Arc::new(Order {
            data: OrderData {
                sell_token: Address::with_last_byte(9),
                sell_amount: alloy::primitives::U256::from(10),
                fee_amount: alloy::primitives::U256::from(5),
                partially_fillable: false,
                ..Default::default()
            },
            metadata: OrderMetadata {
                uid: OrderUid::from_parts(B256::repeat_byte(8), Address::repeat_byte(88), 8),
                ..Default::default()
            },
            ..Default::default()
        });

        let orders = [
            regular_order.clone(),
            eip1271_order.clone(),
            wrapper_order.clone(),
        ];
        let balances: Balances = Default::default(); // No balances

        // EIP-1271 order and wrapper order should be retained, regular order filtered
        let wrapper_set = HashSet::from([wrapper_order_uid]);
        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (alive_orders, _removed_orders) =
            orders_with_balance(orders_ref, &balances, settlement_contract, &wrapper_set);
        assert_eq!(alive_orders.len(), 2);
        assert!(
            alive_orders
                .iter()
                .any(|o| o.metadata.uid == eip1271_order.metadata.uid)
        );
        assert!(
            alive_orders
                .iter()
                .any(|o| o.metadata.uid == wrapper_order.metadata.uid)
        );

        // Without wrapper set, only EIP-1271 order should be retained
        let empty_set: HashSet<OrderUid> = HashSet::new();
        let orders_ref = orders.iter().map(|o| o.as_ref()).collect::<Vec<_>>();
        let (alive_orders, _removed_orders) =
            orders_with_balance(orders_ref, &balances, settlement_contract, &empty_set);
        assert_eq!(alive_orders.len(), 1);
        assert_eq!(alive_orders[0].metadata.uid, eip1271_order.metadata.uid);
    }

    fn test_order(uid_byte: u8, amount: u8) -> domain::Order {
        crate::test_helpers::test_order(uid_byte, amount)
    }

    fn test_price(value: u128) -> domain::auction::Price {
        domain::auction::Price::try_new(eth::Ether::from(eth::U256::from(value))).unwrap()
    }

    fn normalize(mut state: domain::RawAuctionData) -> domain::RawAuctionData {
        state.orders.sort_by_key(|order| order.uid.0);
        state
    }

    fn apply_events(
        previous: domain::RawAuctionData,
        events: &[DeltaEvent],
    ) -> domain::RawAuctionData {
        let mut orders: HashMap<domain::OrderUid, domain::Order> = previous
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

        normalize(domain::RawAuctionData {
            block: previous.block,
            orders: orders.into_values().collect(),
            prices,
            surplus_capturing_jit_order_owners: previous.surplus_capturing_jit_order_owners,
        })
    }

    #[test]
    fn normalized_delta_surface_ignores_non_delta_fields() {
        let mut a = domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10), test_order(2, 20)],
            prices: HashMap::from([(Address::repeat_byte(0xAA).into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: vec![Address::repeat_byte(0x01)],
        };
        let mut b = a.clone();

        a.block = 7;
        b.block = 999;
        b.surplus_capturing_jit_order_owners = vec![Address::repeat_byte(0xFF)];

        assert_eq!(normalized_delta_surface(a), normalized_delta_surface(b));
    }

    #[test]
    fn with_non_delta_fields_uses_full_rebuild_metadata() {
        let reconstructed = domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: HashMap::from([(Address::repeat_byte(0xAA).into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: vec![Address::repeat_byte(0x11)],
        };
        let full_rebuild = domain::RawAuctionData {
            block: 99,
            orders: vec![test_order(1, 10)],
            prices: HashMap::from([(Address::repeat_byte(0xAA).into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: vec![Address::repeat_byte(0x22)],
        };

        let merged = with_non_delta_fields(reconstructed, &full_rebuild);
        assert_eq!(merged.block, 99);
        assert_eq!(
            merged.surplus_capturing_jit_order_owners,
            vec![Address::repeat_byte(0x22)]
        );
    }

    #[test]
    fn diff_solvable_order_inputs_detects_add_update_remove() {
        let prev_uid = domain::OrderUid([1; 56]);
        let upd_uid = domain::OrderUid([2; 56]);
        let add_uid = domain::OrderUid([3; 56]);
        let rem_uid = domain::OrderUid([4; 56]);

        let previous = boundary::SolvableOrders {
            orders: HashMap::from([
                (prev_uid, Arc::new(model::order::Order::default())),
                (
                    upd_uid,
                    Arc::new(model::order::Order {
                        data: model::order::OrderData {
                            valid_to: 1,
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                ),
                (rem_uid, Arc::new(model::order::Order::default())),
            ]),
            quotes: HashMap::new(),
            latest_settlement_block: 0,
            fetched_from_db: chrono::Utc::now(),
        };
        let current = boundary::SolvableOrders {
            orders: HashMap::from([
                (prev_uid, Arc::new(model::order::Order::default())),
                (
                    upd_uid,
                    Arc::new(model::order::Order {
                        data: model::order::OrderData {
                            valid_to: 2,
                            ..Default::default()
                        },
                        ..Default::default()
                    }),
                ),
                (add_uid, Arc::new(model::order::Order::default())),
            ]),
            quotes: HashMap::new(),
            latest_settlement_block: 0,
            fetched_from_db: chrono::Utc::now(),
        };

        let bundle = diff_solvable_order_inputs(Some(&previous), &current);
        assert!(bundle.order_added_candidates.contains(&add_uid));
        assert!(bundle.order_updated_candidates.contains(&upd_uid));
        assert!(bundle.order_removed_candidates.contains(&rem_uid));
    }

    #[test]
    fn diff_solvable_order_inputs_detects_quote_changes_as_updates() {
        let uid = domain::OrderUid([9; 56]);

        let previous = boundary::SolvableOrders {
            orders: HashMap::from([(uid, Arc::new(model::order::Order::default()))]),
            quotes: HashMap::from([(uid, Arc::new(domain::Quote::default()))]),
            latest_settlement_block: 0,
            fetched_from_db: chrono::Utc::now(),
        };
        let current = boundary::SolvableOrders {
            orders: HashMap::from([(uid, Arc::new(model::order::Order::default()))]),
            quotes: HashMap::new(),
            latest_settlement_block: 0,
            fetched_from_db: chrono::Utc::now(),
        };

        let bundle = diff_solvable_order_inputs(Some(&previous), &current);
        assert!(bundle.quote_updated_candidates.contains(&uid));
    }

    #[test]
    fn build_indexed_state_tracks_filtered_sets() {
        let auction = domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: HashMap::from([(Address::repeat_byte(0xAA).into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        };
        let uid = OrderUid::from_parts(B256::repeat_byte(1), Address::repeat_byte(2), 3);
        let invalid = HashMap::from([(uid, OrderFilterReason::InvalidSignature)]);
        let filtered = vec![
            (uid, OrderFilterReason::InFlight),
            (uid, OrderFilterReason::InsufficientBalance),
            (uid, OrderFilterReason::MissingNativePrice),
        ];

        let indexed = build_indexed_state(&auction, &invalid, &filtered);
        assert_eq!(indexed.current_orders_by_uid.len(), 1);
        assert_eq!(indexed.current_prices_by_token.len(), 1);
        assert!(indexed.filtered_invalid.contains(&uid));
        assert!(indexed.filtered_in_flight.contains(&uid));
        assert!(indexed.filtered_no_balance.contains(&uid));
        assert!(indexed.filtered_no_price.contains(&uid));
    }

    #[test]
    fn compute_delta_events_covers_all_event_categories() {
        let token_a = Address::repeat_byte(0xAA);
        let token_b = Address::repeat_byte(0xBB);
        let token_c = Address::repeat_byte(0xCC);

        let previous = domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10), test_order(2, 20)],
            prices: HashMap::from([
                (token_a.into(), test_price(1_000)),
                (token_b.into(), test_price(2_000)),
            ]),
            surplus_capturing_jit_order_owners: Vec::new(),
        };
        let current = domain::RawAuctionData {
            block: 1,
            orders: vec![
                test_order(1, 11), // updated
                test_order(3, 30), // added
            ],
            prices: HashMap::from([
                (token_a.into(), test_price(1_500)), // changed
                (token_c.into(), test_price(3_000)), /* added
                                                      * token_b removed */
            ]),
            surplus_capturing_jit_order_owners: Vec::new(),
        };

        let events = compute_delta_events(Some(&previous), &current);

        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::OrderAdded(order) if order.uid == domain::OrderUid([3; 56])))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::OrderRemoved(uid) if *uid == domain::OrderUid([2; 56])))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::OrderUpdated(order) if order.uid == domain::OrderUid([1; 56])))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::PriceChanged { token, price: Some(_) } if *token == token_a))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::PriceChanged { token, price: None } if *token == token_b))
        );
    }

    #[test]
    fn replay_reconstructs_state_across_randomized_scenarios() {
        let mut seed = 0xDEAD_BEEF_CAFE_BABEu64;
        let mut state = normalize(domain::RawAuctionData {
            block: 1,
            orders: Vec::new(),
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        // Linear congruential generator for deterministic pseudo-random cases.
        let next = |seed: &mut u64| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *seed
        };

        for _ in 0..120 {
            let mut next_state = state.clone();

            // Mutate orders.
            let uid = ((next(&mut seed) % 12) as u8) + 1;
            let choice = next(&mut seed) % 3;
            let mut orders_by_uid: HashMap<domain::OrderUid, domain::Order> = next_state
                .orders
                .into_iter()
                .map(|order| (order.uid, order))
                .collect();
            match choice {
                0 => {
                    orders_by_uid.insert(domain::OrderUid([uid; 56]), test_order(uid, uid));
                }
                1 => {
                    orders_by_uid.remove(&domain::OrderUid([uid; 56]));
                }
                _ => {
                    if orders_by_uid.contains_key(&domain::OrderUid([uid; 56])) {
                        orders_by_uid.insert(
                            domain::OrderUid([uid; 56]),
                            test_order(uid, uid.saturating_add(1)),
                        );
                    }
                }
            }
            next_state.orders = orders_by_uid.into_values().collect();

            // Mutate prices.
            let token = Address::repeat_byte(((next(&mut seed) % 8) as u8) + 1);
            let price_choice = next(&mut seed) % 3;
            match price_choice {
                0 => {
                    next_state.prices.insert(
                        token.into(),
                        test_price(u128::from((next(&mut seed) % 1000) + 1)),
                    );
                }
                1 => {
                    next_state.prices.remove(&token.into());
                }
                _ => {}
            }

            next_state = normalize(next_state);
            let events = compute_delta_events(Some(&state), &next_state);
            let reconstructed = apply_events(state.clone(), &events);

            assert_eq!(reconstructed, next_state);
            state = next_state;
        }
    }

    #[test]
    fn compute_delta_events_from_empty_emits_full_state() {
        let token = Address::repeat_byte(0x11);
        let current = domain::RawAuctionData {
            block: 7,
            orders: vec![test_order(1, 10), test_order(2, 20)],
            prices: HashMap::from([(token.into(), test_price(1234))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        };

        let events = compute_delta_events(None, &current);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, DeltaEvent::OrderAdded(_)))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, DeltaEvent::PriceChanged { price: Some(_), .. }))
                .count(),
            1
        );
    }

    #[test]
    fn compute_delta_events_for_identical_states_is_empty() {
        let state = normalize(domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10), test_order(2, 20)],
            prices: HashMap::from([
                (Address::repeat_byte(1).into(), test_price(1000)),
                (Address::repeat_byte(2).into(), test_price(2000)),
            ]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let events = compute_delta_events(Some(&state), &state);
        assert!(events.is_empty());
    }

    #[test]
    fn compute_delta_events_emits_order_removed_when_order_disappears() {
        let previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = normalize(domain::RawAuctionData {
            block: 2,
            orders: Vec::new(),
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let events = compute_delta_events(Some(&previous), &current);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::OrderRemoved(uid) if *uid == domain::OrderUid([1; 56])))
        );
    }

    #[test]
    fn compute_delta_events_emits_order_added_when_order_reappears() {
        let previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: Vec::new(),
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = normalize(domain::RawAuctionData {
            block: 2,
            orders: vec![test_order(1, 10)],
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let events = compute_delta_events(Some(&previous), &current);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, DeltaEvent::OrderAdded(order) if order.uid == domain::OrderUid([1; 56])))
        );
    }

    #[test]
    fn compute_delta_events_emits_price_remove_and_add_on_transitions() {
        let token = Address::repeat_byte(0xAB);
        let with_price = normalize(domain::RawAuctionData {
            block: 1,
            orders: Vec::new(),
            prices: HashMap::from([(token.into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let without_price = normalize(domain::RawAuctionData {
            block: 2,
            orders: Vec::new(),
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let remove_events = compute_delta_events(Some(&with_price), &without_price);
        assert!(
            remove_events
                .iter()
                .any(|event| matches!(event, DeltaEvent::PriceChanged { token: changed, price: None } if *changed == token))
        );

        let add_events = compute_delta_events(Some(&without_price), &with_price);
        assert!(
            add_events
                .iter()
                .any(|event| matches!(event, DeltaEvent::PriceChanged { token: changed, price: Some(_) } if *changed == token))
        );
    }

    #[test]
    fn compute_delta_events_ordering_is_deterministic_and_stable() {
        let token_a = Address::repeat_byte(0xA1);
        let token_b = Address::repeat_byte(0xB2);

        let previous = normalize(domain::RawAuctionData {
            block: 10,
            orders: vec![test_order(2, 20), test_order(4, 40)],
            prices: HashMap::from([
                (token_a.into(), test_price(100)),
                (token_b.into(), test_price(200)),
            ]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = normalize(domain::RawAuctionData {
            block: 11,
            orders: vec![test_order(1, 10), test_order(2, 21)],
            prices: HashMap::from([(token_b.into(), test_price(250))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let events_first = compute_delta_events(Some(&previous), &current);
        let events_second = compute_delta_events(Some(&previous), &current);
        assert_eq!(events_first, events_second);

        let expected = vec![
            DeltaEvent::OrderAdded(test_order(1, 10)),
            DeltaEvent::OrderUpdated(test_order(2, 21)),
            DeltaEvent::OrderRemoved(domain::OrderUid([4; 56])),
            DeltaEvent::PriceChanged {
                token: token_a,
                price: None,
            },
            DeltaEvent::PriceChanged {
                token: token_b,
                price: Some(test_price(250)),
            },
        ];

        assert_eq!(events_first, expected);
    }

    #[test]
    fn in_flight_entry_and_exit_changes_coverage() {
        let mut indexed = IndexedAuctionState::default();
        indexed
            .current_orders_by_uid
            .insert(domain::OrderUid([1; 56]), test_order(1, 10));
        indexed
            .current_orders_by_uid
            .insert(domain::OrderUid([2; 56]), test_order(2, 20));
        indexed.filtered_in_flight.insert(OrderUid([2; 56]));

        let in_flight = HashSet::from([OrderUid([1; 56])]);

        let mut impacted = HashSet::new();
        impacted.extend(
            in_flight
                .iter()
                .filter(|uid| {
                    indexed
                        .current_orders_by_uid
                        .contains_key(&domain::OrderUid(uid.0))
                })
                .map(|uid| domain::OrderUid(uid.0)),
        );
        impacted.extend(
            indexed
                .filtered_in_flight
                .iter()
                .filter(|uid| !in_flight.contains(uid))
                .map(|uid| domain::OrderUid(uid.0)),
        );

        assert!(impacted.contains(&domain::OrderUid([1; 56])));
        assert!(impacted.contains(&domain::OrderUid([2; 56])));
    }

    #[test]
    fn incremental_projection_matches_full_rebuild_under_randomized_churn() {
        let mut seed = 0xA11CE_C0DE_F00Du64;
        let mut previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: Vec::new(),
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: vec![Address::repeat_byte(1)],
        });

        let next = |seed: &mut u64| {
            *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            *seed
        };

        for i in 0..120 {
            let mut current = previous.clone();
            current.block = i + 2;
            current.surplus_capturing_jit_order_owners = vec![Address::repeat_byte((i % 7) as u8)];

            let uid = ((next(&mut seed) % 10) as u8) + 1;
            match next(&mut seed) % 3 {
                0 => {
                    let mut orders = current
                        .orders
                        .into_iter()
                        .map(|order| (order.uid, order))
                        .collect::<HashMap<_, _>>();
                    orders.insert(domain::OrderUid([uid; 56]), test_order(uid, uid));
                    current.orders = orders.into_values().collect();
                }
                1 => {
                    current
                        .orders
                        .retain(|order| order.uid != domain::OrderUid([uid; 56]));
                }
                _ => {
                    for order in &mut current.orders {
                        if order.uid == domain::OrderUid([uid; 56]) {
                            *order = test_order(uid, uid.saturating_add(1));
                        }
                    }
                }
            }

            let token = Address::repeat_byte(((next(&mut seed) % 8) as u8) + 1);
            match next(&mut seed) % 3 {
                0 => {
                    current.prices.insert(
                        token.into(),
                        test_price(u128::from((next(&mut seed) % 2000) + 1)),
                    );
                }
                1 => {
                    current.prices.remove(&token.into());
                }
                _ => {}
            }

            current = normalize(current);
            let events = compute_delta_events(Some(&previous), &current);
            let reconstructed = apply_delta_events_to_auction(previous.clone(), &events);
            let merged = with_non_delta_fields(reconstructed, &current);

            assert_eq!(
                normalized_delta_surface(merged),
                normalized_delta_surface(current.clone())
            );
            assert_eq!(events, compute_delta_events(Some(&previous), &current));
            previous = current;
        }
    }

    #[test]
    fn compute_delta_events_from_inputs_falls_back_to_canonical_when_bundle_is_wrong() {
        let previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: HashMap::from([(Address::repeat_byte(0xAA).into(), test_price(1000))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = normalize(domain::RawAuctionData {
            block: 2,
            orders: vec![test_order(1, 11), test_order(2, 20)],
            prices: HashMap::from([(Address::repeat_byte(0xAA).into(), test_price(1500))]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        // Intentionally wrong: claims order 1 was removed and misses order 2 add.
        let noisy_bundle = ChangeBundle {
            order_added_candidates: Vec::new(),
            order_removed_candidates: vec![domain::OrderUid([1; 56])],
            order_updated_candidates: Vec::new(),
            quote_updated_candidates: Vec::new(),
            price_changed_tokens: vec![Address::repeat_byte(0xAA)],
            filter_transitions: Vec::new(),
        };

        let events = SolvableOrdersCache::compute_delta_events_from_inputs(
            Some(&previous),
            &current,
            &noisy_bundle,
            true,
        )
        .unwrap();
        let canonical = compute_delta_events(Some(&previous), &current);
        assert_eq!(events, canonical);
    }

    #[test]
    fn compute_delta_events_from_inputs_dedups_candidates_and_skips_unchanged_price_tokens() {
        let previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: HashMap::from([
                (Address::repeat_byte(0xAA).into(), test_price(1000)),
                (Address::repeat_byte(0xBB).into(), test_price(2000)),
            ]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = normalize(domain::RawAuctionData {
            block: 2,
            orders: vec![test_order(1, 11)],
            prices: HashMap::from([
                (Address::repeat_byte(0xAA).into(), test_price(1500)),
                (Address::repeat_byte(0xBB).into(), test_price(2000)),
            ]),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let bundle = ChangeBundle {
            order_added_candidates: vec![],
            order_removed_candidates: vec![],
            order_updated_candidates: vec![domain::OrderUid([1; 56]), domain::OrderUid([1; 56])],
            quote_updated_candidates: Vec::new(),
            price_changed_tokens: vec![
                Address::repeat_byte(0xAA),
                Address::repeat_byte(0xAA),
                Address::repeat_byte(0xBB),
            ],
            filter_transitions: Vec::new(),
        };

        let events = SolvableOrdersCache::compute_delta_events_from_inputs(
            Some(&previous),
            &current,
            &bundle,
            false,
        )
        .unwrap();

        assert_eq!(
            events,
            vec![
                DeltaEvent::OrderUpdated(test_order(1, 11)),
                DeltaEvent::PriceChanged {
                    token: Address::repeat_byte(0xAA),
                    price: Some(test_price(1500)),
                },
            ]
        );
    }

    #[test]
    fn compute_delta_events_from_inputs_emits_single_event_per_uid() {
        let uid = domain::OrderUid([9; 56]);
        let previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(9, 10)],
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = normalize(domain::RawAuctionData {
            block: 2,
            orders: vec![test_order(9, 11)],
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });

        let bundle = ChangeBundle {
            order_added_candidates: vec![uid],
            order_removed_candidates: vec![uid],
            order_updated_candidates: vec![uid],
            quote_updated_candidates: Vec::new(),
            price_changed_tokens: Vec::new(),
            filter_transitions: vec![FilterTransition {
                uid,
                reason: OrderFilterReason::InFlight,
                is_filtered: true,
            }],
        };

        let events = SolvableOrdersCache::compute_delta_events_from_inputs(
            Some(&previous),
            &current,
            &bundle,
            false,
        )
        .unwrap();

        let uid_events = events
            .iter()
            .filter(|event| match event {
                DeltaEvent::OrderAdded(order) | DeltaEvent::OrderUpdated(order) => order.uid == uid,
                DeltaEvent::OrderRemoved(event_uid) => *event_uid == uid,
                _ => false,
            })
            .count();
        assert_eq!(uid_events, 1);
    }

    #[test]
    fn compute_delta_events_from_inputs_skips_unchanged_solver_visible_fields() {
        let uid = domain::OrderUid([1; 56]);
        let previous = normalize(domain::RawAuctionData {
            block: 1,
            orders: vec![test_order(1, 10)],
            prices: HashMap::new(),
            surplus_capturing_jit_order_owners: Vec::new(),
        });
        let current = previous.clone();

        let bundle = ChangeBundle {
            order_added_candidates: Vec::new(),
            order_removed_candidates: Vec::new(),
            order_updated_candidates: vec![uid],
            quote_updated_candidates: Vec::new(),
            price_changed_tokens: Vec::new(),
            filter_transitions: Vec::new(),
        };

        let events = SolvableOrdersCache::compute_delta_events_from_inputs(
            Some(&previous),
            &current,
            &bundle,
            false,
        )
        .unwrap();

        assert!(events.is_empty());
    }

    #[test]
    fn apply_auction_id_change_keeps_sequence_monotonic() {
        let mut inner = Inner {
            auction: domain::RawAuctionData {
                block: 1,
                orders: Vec::new(),
                prices: HashMap::new(),
                surplus_capturing_jit_order_owners: Vec::new(),
            },
            solvable_orders: boundary::SolvableOrders {
                orders: HashMap::new(),
                quotes: HashMap::new(),
                latest_settlement_block: 0,
                fetched_from_db: chrono::Utc::now(),
            },
            auction_id: 1,
            auction_sequence: 7,
            delta_sequence: 9,
            delta_history: VecDeque::new(),
            indexed_state: Arc::new(IndexedAuctionState::default()),
        };

        let envelope = apply_auction_id_change(&mut inner, 2).expect("envelope expected");
        assert_eq!(envelope.from_sequence, 9);
        assert_eq!(envelope.to_sequence, 10);
        assert_eq!(inner.delta_sequence, 10);

        let envelope = apply_auction_id_change(&mut inner, 3).expect("envelope expected");
        assert_eq!(envelope.from_sequence, 10);
        assert_eq!(envelope.to_sequence, 11);
        assert_eq!(inner.delta_sequence, 11);
    }

    #[test]
    fn apply_auction_id_change_returns_none_when_unchanged() {
        let mut inner = Inner {
            auction: domain::RawAuctionData {
                block: 1,
                orders: Vec::new(),
                prices: HashMap::new(),
                surplus_capturing_jit_order_owners: Vec::new(),
            },
            solvable_orders: boundary::SolvableOrders {
                orders: HashMap::new(),
                quotes: HashMap::new(),
                latest_settlement_block: 0,
                fetched_from_db: chrono::Utc::now(),
            },
            auction_id: 5,
            auction_sequence: 2,
            delta_sequence: 3,
            delta_history: VecDeque::new(),
            indexed_state: Arc::new(IndexedAuctionState::default()),
        };

        let previous_sequence = inner.delta_sequence;
        assert!(apply_auction_id_change(&mut inner, 5).is_none());
        assert_eq!(inner.delta_sequence, previous_sequence);
        assert!(inner.delta_history.is_empty());
    }

    #[cfg(debug_assertions)]
    #[tokio::test]
    #[should_panic(expected = "update_lock must be held when mutating cache")]
    async fn assert_update_lock_held_panics_without_lock() {
        let cache = test_cache().await;
        cache.assert_update_lock_held();
    }

    #[tokio::test]
    async fn assert_update_lock_held_allows_guarded_access() {
        let cache = test_cache().await;
        let _guard = cache.update_lock.lock().await;
        cache.assert_update_lock_held();
    }

    #[test]
    fn delta_replay_includes_gap_envelope() {
        let envelope = DeltaEnvelope {
            auction_id: 7,
            auction_sequence: 2,
            from_sequence: 1,
            to_sequence: 2,
            published_at: chrono::Utc::now(),
            published_at_instant: Instant::now(),
            events: vec![DeltaEvent::OrderAdded(test_order(1, 10))],
        };
        let inner = Inner {
            auction: domain::RawAuctionData {
                block: 1,
                orders: vec![test_order(1, 10)],
                prices: HashMap::new(),
                surplus_capturing_jit_order_owners: Vec::new(),
            },
            solvable_orders: boundary::SolvableOrders {
                orders: HashMap::new(),
                quotes: HashMap::new(),
                latest_settlement_block: 0,
                fetched_from_db: chrono::Utc::now(),
            },
            auction_id: 7,
            auction_sequence: 2,
            delta_sequence: 2,
            delta_history: VecDeque::from([envelope.clone()]),
            indexed_state: Arc::new(IndexedAuctionState::default()),
        };

        let replay = SolvableOrdersCache::build_delta_replay(1, Some(&inner)).unwrap();
        assert_eq!(replay.checkpoint_sequence, 2);
        assert_eq!(replay.envelopes.len(), 1);
        assert_eq!(replay.envelopes[0].to_sequence, 2);
    }

    #[test]
    fn delta_replay_includes_auction_changed_envelope() {
        let envelope = DeltaEnvelope {
            auction_id: 8,
            auction_sequence: 0,
            from_sequence: 3,
            to_sequence: 4,
            published_at: chrono::Utc::now(),
            published_at_instant: Instant::now(),
            events: vec![DeltaEvent::AuctionChanged { new_auction_id: 8 }],
        };
        let inner = Inner {
            auction: domain::RawAuctionData {
                block: 1,
                orders: Vec::new(),
                prices: HashMap::new(),
                surplus_capturing_jit_order_owners: Vec::new(),
            },
            solvable_orders: boundary::SolvableOrders {
                orders: HashMap::new(),
                quotes: HashMap::new(),
                latest_settlement_block: 0,
                fetched_from_db: chrono::Utc::now(),
            },
            auction_id: 8,
            auction_sequence: 0,
            delta_sequence: 4,
            delta_history: VecDeque::from([envelope.clone()]),
            indexed_state: Arc::new(IndexedAuctionState::default()),
        };

        let replay = SolvableOrdersCache::build_delta_replay(3, Some(&inner)).unwrap();
        assert_eq!(replay.envelopes.len(), 1);
        assert!(matches!(
            replay.envelopes[0].events.first(),
            Some(DeltaEvent::AuctionChanged { new_auction_id: 8 })
        ));
    }

    #[test]
    fn prune_delta_history_evicts_by_max_count() {
        let now = chrono::Utc::now();
        let instant_now = Instant::now();
        let mut delta_history = VecDeque::new();

        for i in 0..(MAX_DELTA_HISTORY + 5) {
            delta_history.push_back(DeltaEnvelope {
                auction_id: 1,
                auction_sequence: i as u64,
                from_sequence: i as u64,
                to_sequence: (i + 1) as u64,
                published_at: now,
                published_at_instant: instant_now,
                events: vec![DeltaEvent::OrderAdded(test_order(1, 10))],
            });
        }

        prune_delta_history(&mut delta_history, chrono::Duration::seconds(600));

        assert_eq!(delta_history.len(), MAX_DELTA_HISTORY);
        assert_eq!(delta_history.front().unwrap().from_sequence, 5_u64);
    }

    #[test]
    fn prune_delta_history_evicts_by_age() {
        let _guard = EnvGuard::set("AUTOPILOT_DELTA_SYNC_HISTORY_MIN_RETAINED", "1");

        let now = chrono::Utc::now();
        let instant_now = Instant::now();
        let mut delta_history = VecDeque::from([
            DeltaEnvelope {
                auction_id: 1,
                auction_sequence: 1,
                from_sequence: 1,
                to_sequence: 2,
                published_at: now - chrono::Duration::seconds(120),
                published_at_instant: instant_now - Duration::from_secs(120),
                events: vec![DeltaEvent::OrderAdded(test_order(1, 10))],
            },
            DeltaEnvelope {
                auction_id: 1,
                auction_sequence: 2,
                from_sequence: 2,
                to_sequence: 3,
                published_at: now,
                published_at_instant: instant_now,
                events: vec![DeltaEvent::OrderAdded(test_order(2, 20))],
            },
        ]);
        prune_delta_history(&mut delta_history, chrono::Duration::seconds(60));

        assert_eq!(delta_history.len(), 1);
        assert_eq!(delta_history.front().unwrap().to_sequence, 3);
    }
}
