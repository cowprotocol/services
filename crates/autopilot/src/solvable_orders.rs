use {
    crate::{
        boundary::{self, SolvableOrders},
        domain::{self, auction::Price},
        infra::{self, banned},
    },
    account_balances::{BalanceFetching, Query},
    alloy::primitives::{
        Address,
        U256,
        map::{AddressHashSet, FbBuildHasher},
    },
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
    model::{
        order::{Order, OrderUid},
        signature::Signature,
        time::now_in_epoch_seconds,
    },
    price_estimation::{native::to_normalized_price, native_price_cache::NativePriceUpdater},
    prometheus::{
        Histogram,
        HistogramVec,
        IntCounter,
        IntCounterVec,
        IntGaugeVec,
        core::{AtomicU64, GenericGauge},
    },
    shared::remaining_amounts,
    std::{
        collections::{HashMap, HashSet},
        future::Future,
        sync::Arc,
        time::{Duration, Instant},
    },
    tokio::sync::Mutex,
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

    /// Auction solvable orders.
    auction_solvable_orders: GenericGauge<AtomicU64>,

    /// Auction filtered orders grouped by class.
    #[metric(labels("reason"))]
    auction_filtered_orders: IntGaugeVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    #[instrument(skip_all)]
    fn track_filtered_orders(reason: OrderFilterReason, invalid_orders: &[OrderUid]) {
        Metrics::get()
            .auction_filtered_orders
            .with_label_values(&[reason.as_str()])
            .set(i64::try_from(invalid_orders.len()).unwrap_or(i64::MAX));

        if !invalid_orders.is_empty() {
            // clone and print in background task?
            tracing::debug!(
                %reason,
                count = invalid_orders.len(),
                orders = ?invalid_orders, "filtered orders"
            );
        }
    }

    #[instrument(skip_all)]
    fn track_orders_in_final_auction(orders: &[domain::Order]) {
        let metrics = Metrics::get();
        metrics.auction_creations.inc();
        metrics.auction_solvable_orders.set(orders.len() as u64);
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
    native_price_estimator: Arc<NativePriceUpdater>,
    weth: Address,
    protocol_fees: domain::ProtocolFees,
    surplus_capturing_jit_order_owners: Vec<Address>,
    disable_order_balance_filter: bool,
    wrapper_cache: app_data::WrapperCache,
}

struct Inner {
    auction: domain::RawAuctionData,
    solvable_orders: boundary::SolvableOrders,
}

/// Orders dropped during a single `update()` cycle, grouped by the reason they
/// were dropped. Owns the ability to both emit filtered-order metrics and
/// persist the events, keeping observability separate from the filtering logic.
#[derive(Default)]
struct FilteredOrders {
    token_deny_listed: Vec<OrderUid>,
    presig_pending: Vec<OrderUid>,
    missing_price: Vec<OrderUid>,
    in_flight: Vec<OrderUid>,
    banned_user: Vec<OrderUid>,
    insufficient_balance: Vec<OrderUid>,
    dust: Vec<OrderUid>,
}

impl FilteredOrders {
    /// Handles all the observability (metrics, logging, upload order events).
    fn report(self, persistence: &infra::Persistence, store_events: bool) {
        Metrics::track_filtered_orders(UnsupportedToken, &self.token_deny_listed);
        Metrics::track_filtered_orders(InvalidSignature, &self.presig_pending);
        Metrics::track_filtered_orders(MissingNativePrice, &self.missing_price);
        Metrics::track_filtered_orders(InFlight, &self.in_flight);
        Metrics::track_filtered_orders(BannedUser, &self.banned_user);
        Metrics::track_filtered_orders(InsufficientBalance, &self.insufficient_balance);
        Metrics::track_filtered_orders(DustOrder, &self.dust);

        if store_events {
            self.store_order_events(persistence)
        }
    }

    /// Uploads order debug events to the database in separate
    /// background tasks.
    fn store_order_events(self, persistence: &infra::Persistence) {
        let store = |uids: Vec<OrderUid>, label, reason| {
            if uids.is_empty() {
                return;
            }
            persistence.store_order_events_owned(
                uids,
                |uid| domain::OrderUid(uid.0),
                label,
                Some(reason),
            );
        };
        store(
            self.token_deny_listed,
            OrderEventLabel::Invalid,
            UnsupportedToken,
        );
        store(
            self.presig_pending,
            OrderEventLabel::Invalid,
            InvalidSignature,
        );
        store(self.banned_user, OrderEventLabel::Invalid, BannedUser);
        store(
            self.insufficient_balance,
            OrderEventLabel::Invalid,
            InsufficientBalance,
        );
        store(self.in_flight, OrderEventLabel::Filtered, InFlight);
        store(self.dust, OrderEventLabel::Filtered, DustOrder);
        store(
            self.missing_price,
            OrderEventLabel::Filtered,
            MissingNativePrice,
        );
    }
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
        surplus_capturing_jit_order_owners: Vec<Address>,
        disable_order_balance_filter: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            min_order_validity_period,
            persistence,
            banned_users,
            balance_fetcher,
            deny_listed_tokens,
            cache: Mutex::new(None),
            native_price_estimator,
            weth,
            protocol_fees,
            surplus_capturing_jit_order_owners,
            disable_order_balance_filter,
            wrapper_cache: app_data::WrapperCache::new(20_000),
        })
    }

    pub async fn current_auction(&self) -> Option<domain::RawAuctionData> {
        self.cache
            .lock()
            .await
            .as_ref()
            .map(|inner| inner.auction.clone())
    }

    /// Manually update solvable orders. Usually called by the background
    /// updating task.
    ///
    /// Usually this method is called from update_task. If it isn't, which is
    /// the case in unit tests, then concurrent calls might overwrite each
    /// other's results.
    #[instrument(skip_all)]
    pub async fn update(&self, block: u64, store_events: bool) -> Result<()> {
        let start = Instant::now();

        let _timer = observe::metrics::metrics()
            .on_auction_overhead_start("autopilot", "update_solvabe_orders");

        let (db_solvable_orders, in_flight) = tokio::try_join!(
            self.get_solvable_orders(),
            self.fetch_in_flight_orders(block).map(Ok),
        )?;
        tracing::trace!("fetched solvable orders from db");

        // Exclude any owner that already has an order in-flight (i.e. won a previous
        // auction and is being settled on-chain). A surplus-capturing JIT order created
        // on its behalf could conflict with the settling order, so we drop the owner
        // from this auction until the in-flight order clears.
        let surplus_capturing_jit_order_owners: Vec<Address> = {
            let in_flight_owners: AddressHashSet = in_flight
                .iter()
                .map(|uid| domain::OrderUid(uid.0).owner())
                .collect();
            self.surplus_capturing_jit_order_owners
                .iter()
                .filter(|owner| !in_flight_owners.contains(*owner))
                .copied()
                .collect()
        };

        // Phase 1: single-pass sync pre-filter that also collects everything
        // needed for the concurrent I/O in phase 2.
        let mut prices = self.native_price_estimator.cached_prices();
        // WETH's native price is 1 by definition — insert it directly to
        // support ETH wrap when required.
        prices
            .entry(self.weth)
            .or_insert_with(|| to_normalized_price(1.0).unwrap());

        let capacity_hint = db_solvable_orders.orders.len();
        let mut filtered = FilteredOrders::default();

        // data needed for filtering logic in next phase
        let mut traders = AddressHashSet::default();
        let mut balance_queries = Vec::with_capacity(capacity_hint);
        let mut traded_tokens = AddressHashSet::default();
        let mut balance_filter_exempt = HashSet::<OrderUid, FbBuildHasher<56>>::default();
        let mut survivors: Vec<&Order> = Vec::with_capacity(capacity_hint);

        for order in db_solvable_orders.orders.values() {
            let order = order.as_ref();
            let uid = order.metadata.uid;

            // store tokens even for discarded orders to later inform the native price
            // cache about ALL the tokens we need prices for
            traded_tokens.insert(order.data.sell_token);
            traded_tokens.insert(order.data.buy_token);

            if in_flight.contains(&uid) {
                filtered.in_flight.push(uid);
                continue;
            }
            if token_deny_listed(order, &self.deny_listed_tokens) {
                filtered.token_deny_listed.push(uid);
                continue;
            }
            if is_presig_pending(order) {
                filtered.presig_pending.push(uid);
                continue;
            }
            if !prices.contains_key(&order.data.sell_token)
                || !prices.contains_key(&order.data.buy_token)
            {
                filtered.missing_price.push(uid);
                continue;
            }

            traders.insert(order.metadata.owner);
            if let Some(receiver) = order.data.receiver {
                traders.insert(receiver);
            }
            if !self.disable_order_balance_filter {
                balance_queries.push(Query::from_order(order));
                if self.wrapper_cache.has_wrappers(
                    &order.data.app_data,
                    order.metadata.full_app_data.as_deref(),
                ) {
                    balance_filter_exempt.insert(uid);
                }
            }
            survivors.push(order);
        }

        // at this point we know all relevant tokens and tell the native price
        // cache to have them ready for the next auction
        self.native_price_estimator
            .schedule_token_updates(traded_tokens);

        // Phase 2: concurrent I/O based on phase-1 outputs.
        let (banned_set, balances) = tokio::join!(
            self.timed_future("banned_user_filtering", self.banned_users.banned(traders)),
            self.fetch_balances(balance_queries),
        );

        // Phase 3: final pass using data from phase-2
        let final_orders = survivors
            .into_iter()
            .filter_map(|order| {
                let uid = order.metadata.uid;
                let is_banned = banned_set.contains(&order.metadata.owner)
                    || order.data.receiver.is_some_and(|r| banned_set.contains(&r));
                if is_banned {
                    filtered.banned_user.push(uid);
                    return None;
                }
                if !self.disable_order_balance_filter {
                    let Some(&balance) = balances.get(&Query::from_order(order)) else {
                        filtered.insufficient_balance.push(uid);
                        return None;
                    };

                    if !passes_balance(order, balance, &balance_filter_exempt) {
                        filtered.insufficient_balance.push(uid);
                        return None;
                    }
                    if !passes_dust(order, balance) {
                        filtered.dust.push(uid);
                        return None;
                    }
                }

                let quote = db_solvable_orders
                    .quotes
                    .get(&order.metadata.uid.into())
                    .map(|quote| quote.as_ref().clone());
                let final_order =
                    self.protocol_fees
                        .apply(order, quote, &surplus_capturing_jit_order_owners);
                Some(final_order)
            })
            .collect::<Vec<_>>();

        Metrics::track_orders_in_final_auction(&final_orders);
        filtered.report(&self.persistence, store_events);

        let auction = domain::RawAuctionData {
            block,
            orders: final_orders,
            prices: prices
                .into_iter()
                .map(|(key, value)| Price::try_new(value.into()).map(|price| (key.into(), price)))
                .collect::<Result<_, _>>()?,
            surplus_capturing_jit_order_owners,
        };

        *self.cache.lock().await = Some(Inner {
            auction,
            solvable_orders: db_solvable_orders,
        });

        tracing::debug!(%block, "updated current auction cache");
        Metrics::get()
            .auction_update_total_time
            .observe(start.elapsed().as_secs_f64());
        Ok(())
    }

    async fn fetch_in_flight_orders(&self, block: u64) -> HashSet<OrderUid, FbBuildHasher<56>> {
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
        let fetch_orders = {
            let lock = self.cache.lock().await;
            match &*lock {
                // Only use incremental query after cache already got initialized
                // because it's not optimized for very long durations.
                Some(cache) => self
                    .persistence
                    .solvable_orders_after(
                        cache.solvable_orders.orders.clone(),
                        cache.solvable_orders.quotes.clone(),
                        cache.solvable_orders.fetched_from_db,
                        cache.solvable_orders.latest_settlement_block,
                        min_valid_to,
                    )
                    .boxed(),
                None => self.persistence.all_solvable_orders(min_valid_to).boxed(),
            }
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
}

/// Returns true if either of the order's tokens is on the deny list.
#[inline(always)]
fn token_deny_listed(order: &Order, deny_listed_tokens: &DenyListedTokens) -> bool {
    deny_listed_tokens.contains(&order.data.sell_token)
        || deny_listed_tokens.contains(&order.data.buy_token)
}

/// Returns true if the order is waiting for a pre-signature. EIP-1271 orders
/// are validated by the driver before settlement, so we don't check them here.
#[inline(always)]
fn is_presig_pending(order: &Order) -> bool {
    matches!(
        order.metadata.status,
        model::order::OrderStatus::PresignaturePending
    )
}

/// Returns true if the order has sufficient balance to be settled. EIP-1271
/// orders and orders exempt via a wrapper interaction bypass the check.
#[inline(always)]
fn passes_balance(
    order: &Order,
    balance: U256,
    exempt: &HashSet<OrderUid, FbBuildHasher<56>>,
) -> bool {
    // EIP-1271 orders can unlock funds via pre-interactions; wrapper orders
    // produce the required balance at settlement time.
    if matches!(order.signature, Signature::Eip1271(_)) || exempt.contains(&order.metadata.uid) {
        return true;
    }

    if order.data.partially_fillable && !balance.is_zero() {
        return true;
    }

    let Some(needed_balance) = order.data.sell_amount.checked_add(order.data.fee_amount) else {
        return false;
    };
    balance >= needed_balance
}

/// Returns true if the order is not a dust order —  its remaining sell and
/// buy amounts (scaled by balance) are both non-zero.
#[inline(always)]
fn passes_dust(order: &Order, balance: U256) -> bool {
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
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{Address, B256},
        bad_tokens::list_based::DenyListedTokens,
        model::order::{OrderBuilder, OrderData, OrderMetadata, OrderUid},
    };

    #[test]
    fn is_presig_pending_only_matches_presig() {
        let presig = Order {
            metadata: OrderMetadata {
                status: model::order::OrderStatus::PresignaturePending,
                ..Default::default()
            },
            ..Default::default()
        };
        let eip1271 = Order {
            signature: Signature::Eip1271(vec![2, 2]),
            ..Default::default()
        };
        let regular = Order::default();

        assert!(is_presig_pending(&presig));
        assert!(!is_presig_pending(&eip1271));
        assert!(!is_presig_pending(&regular));
    }

    #[test]
    fn is_unsupported_matches_either_side() {
        let token0 = Address::with_last_byte(0);
        let token1 = Address::with_last_byte(1);
        let token2 = Address::with_last_byte(2);
        let deny_listed_tokens = DenyListedTokens::new(vec![token0]);

        let sell_denied = OrderBuilder::default()
            .with_sell_token(token0)
            .with_buy_token(token1)
            .build();
        let neither_denied = OrderBuilder::default()
            .with_sell_token(token1)
            .with_buy_token(token2)
            .build();
        let buy_denied = OrderBuilder::default()
            .with_sell_token(token1)
            .with_buy_token(token0)
            .build();

        assert!(token_deny_listed(&sell_denied, &deny_listed_tokens));
        assert!(!token_deny_listed(&neither_denied, &deny_listed_tokens));
        assert!(token_deny_listed(&buy_denied, &deny_listed_tokens));
    }

    #[test]
    fn passes_balance_covers_all_cases() {
        let orders = [
            // enough balance for sell and fee
            Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(2),
                    sell_amount: alloy::primitives::U256::ONE,
                    fee_amount: alloy::primitives::U256::ONE,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            // missing fee balance
            Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(3),
                    sell_amount: alloy::primitives::U256::ONE,
                    fee_amount: alloy::primitives::U256::ONE,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            // at least 1 partially fillable balance
            Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(4),
                    sell_amount: alloy::primitives::U256::from(2),
                    fee_amount: alloy::primitives::U256::ZERO,
                    partially_fillable: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            // 0 partially fillable balance
            Order {
                data: OrderData {
                    sell_token: Address::with_last_byte(5),
                    sell_amount: alloy::primitives::U256::from(2),
                    fee_amount: alloy::primitives::U256::ZERO,
                    partially_fillable: true,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        let no_bypass = HashSet::with_hasher(FbBuildHasher::default());

        assert!(passes_balance(&orders[0], U256::from(2), &no_bypass));
        assert!(!passes_balance(&orders[1], U256::ONE, &no_bypass));
        assert!(passes_balance(&orders[2], U256::ONE, &no_bypass));
        assert!(!passes_balance(&orders[3], U256::ZERO, &no_bypass));
    }

    #[test]
    fn passes_balance_bypasses_eip1271_and_wrappers() {
        let eip1271_order = Order {
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
        };

        let wrapper_order_uid =
            OrderUid::from_parts(B256::repeat_byte(7), Address::repeat_byte(77), 7);
        let wrapper_order = Order {
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
        };

        let regular_order = Order {
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
        };

        let wrapper_set: HashSet<OrderUid, FbBuildHasher<56>> =
            [wrapper_order_uid].into_iter().collect();
        let empty_set = HashSet::with_hasher(FbBuildHasher::default());

        // EIP-1271 always bypasses regardless of the exempt set.
        assert!(passes_balance(&eip1271_order, U256::ZERO, &empty_set));
        assert!(passes_balance(&eip1271_order, U256::ZERO, &wrapper_set));

        // Wrapper order bypasses only when its uid is in the exempt set.
        assert!(!passes_balance(&wrapper_order, U256::ZERO, &empty_set));
        assert!(passes_balance(&wrapper_order, U256::ZERO, &wrapper_set));

        // Regular order without a matching balance entry always fails.
        assert!(!passes_balance(&regular_order, U256::ZERO, &empty_set));
        assert!(!passes_balance(&regular_order, U256::ZERO, &wrapper_set));
    }
}
