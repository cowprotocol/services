use {
    crate::{
        boundary::{self, SolvableOrders},
        domain::{self, auction::Price, eth},
        infra::{self, banned},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    database::order_events::OrderEventLabel,
    futures::FutureExt,
    itertools::Itertools,
    model::{
        order::{Order, OrderClass, OrderUid},
        signature::Signature,
        time::now_in_epoch_seconds,
    },
    prometheus::{Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec},
    shared::{
        account_balances::{BalanceFetching, Query},
        bad_token::list_based::DenyListedTokens,
        price_estimation::{
            native::{NativePriceEstimating, to_normalized_price},
            native_price_cache::NativePriceUpdater,
        },
        remaining_amounts,
    },
    std::{
        collections::{BTreeMap, HashMap, HashSet, btree_map::Entry},
        future::Future,
        sync::Arc,
        time::{Duration, Instant},
    },
    strum::VariantNames,
    tokio::sync::Mutex,
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
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    fn track_filtered_orders(reason: &'static str, invalid_orders: &[OrderUid]) {
        if invalid_orders.is_empty() {
            return;
        }

        Metrics::get()
            .auction_filtered_orders
            .with_label_values(&[reason])
            .set(i64::try_from(invalid_orders.len()).unwrap_or(i64::MAX));

        tracing::debug!(
            %reason,
            count = invalid_orders.len(),
            orders = ?invalid_orders, "filtered orders"
        );
    }

    fn track_orders_in_final_auction(orders: &[Arc<Order>]) {
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
    native_price_estimator: Arc<NativePriceUpdater>,
    weth: Address,
    protocol_fees: domain::ProtocolFees,
    cow_amm_registry: cow_amm::Registry,
    native_price_timeout: Duration,
    settlement_contract: Address,
    disable_order_balance_filter: bool,
    wrapper_cache: app_data::WrapperCache,
}

type Balances = HashMap<Query, U256>;

struct Inner {
    auction: domain::RawAuctionData,
    solvable_orders: boundary::SolvableOrders,
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
            cow_amm_registry,
            native_price_timeout,
            settlement_contract,
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
    pub async fn update(&self, block: u64, store_events: bool) -> Result<()> {
        let start = Instant::now();

        let _timer = observe::metrics::metrics()
            .on_auction_overhead_start("autopilot", "update_solvabe_orders");

        let db_solvable_orders = self.get_solvable_orders().await?;
        tracing::trace!("fetched solvable orders from db");

        let orders = db_solvable_orders
            .orders
            .values()
            .cloned()
            .collect::<Vec<_>>();

        let mut invalid_order_uids = HashSet::new();
        let mut filtered_order_events = Vec::new();

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

        let (balances, orders, cow_amms) = {
            let queries = orders
                .iter()
                .map(|o| Query::from_order(o.as_ref()))
                .collect::<Vec<_>>();
            tokio::join!(
                self.fetch_balances(queries),
                self.filter_invalid_orders(orders, &mut invalid_order_uids),
                self.timed_future("cow_amm_registry", self.cow_amm_registry.amms()),
            )
        };

        let orders = if self.disable_order_balance_filter {
            orders
        } else {
            let (orders, removed) = orders_with_balance(
                orders,
                &balances,
                self.settlement_contract,
                &balance_filter_exempt_orders,
            );
            Metrics::track_filtered_orders("insufficient_balance", &removed);
            invalid_order_uids.extend(removed);

            let (orders, removed) = filter_dust_orders(orders, &balances);
            Metrics::track_filtered_orders("dust_order", &removed);
            filtered_order_events.extend(removed);

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
        Metrics::track_filtered_orders("missing_price", &removed);
        filtered_order_events.extend(removed);

        Metrics::track_orders_in_final_auction(&orders);

        if store_events {
            // spawning a background task since `order_events` table insert operation takes
            // a while and the result is ignored.
            self.persistence.store_order_events_owned(
                invalid_order_uids,
                |uid| domain::OrderUid(uid.0),
                OrderEventLabel::Invalid,
            );
            self.persistence.store_order_events_owned(
                filtered_order_events,
                |uid| domain::OrderUid(uid.0),
                OrderEventLabel::Filtered,
            );
        }

        let surplus_capturing_jit_order_owners = cow_amms
            .iter()
            .filter(|cow_amm| {
                cow_amm.traded_tokens().iter().all(|token| {
                    let price_exist = prices.contains_key(token);
                    if !price_exist {
                        tracing::debug!(
                            cow_amm = ?cow_amm.address(),
                            ?token,
                            "omitted from auction due to missing prices"
                        );
                    }
                    price_exist
                })
            })
            .map(|cow_amm| *cow_amm.address())
            .collect::<Vec<_>>();
        let auction = domain::RawAuctionData {
            block,
            orders: orders
                .into_iter()
                .map(|order| {
                    let quote = db_solvable_orders
                        .quotes
                        .get(&order.metadata.uid.into())
                        .map(|quote| quote.as_ref().clone());
                    self.protocol_fees.apply(
                        order.as_ref(),
                        quote,
                        &surplus_capturing_jit_order_owners,
                    )
                })
                .collect(),
            prices: prices
                .into_iter()
                .map(|(key, value)| {
                    Price::try_new(value.into()).map(|price| (eth::TokenAddress(key), price))
                })
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
    async fn filter_invalid_orders(
        &self,
        mut orders: Vec<Arc<Order>>,
        invalid_order_uids: &mut HashSet<OrderUid>,
    ) -> Vec<Arc<Order>> {
        let presignature_pending_orders = find_presignature_pending_orders(&orders);

        let unsupported_token_orders = find_unsupported_tokens(&orders, &self.deny_listed_tokens);
        let banned_user_orders = self
            .timed_future(
                "banned_user_filtering",
                find_banned_user_orders(&orders, &self.banned_users),
            )
            .await;
        tracing::trace!("filtered invalid orders");

        Metrics::track_filtered_orders("banned_user", &banned_user_orders);
        Metrics::track_filtered_orders("invalid_signature", &presignature_pending_orders);
        Metrics::track_filtered_orders("unsupported_token", &unsupported_token_orders);
        invalid_order_uids.extend(banned_user_orders);
        invalid_order_uids.extend(presignature_pending_orders);
        invalid_order_uids.extend(unsupported_token_orders);

        orders.retain(|order| !invalid_order_uids.contains(&order.metadata.uid));
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
}

/// Finds all orders whose owners or receivers are in the set of "banned"
/// users.
async fn find_banned_user_orders(
    orders: &[Arc<Order>],
    banned_users: &banned::Users,
) -> Vec<OrderUid> {
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
fn find_presignature_pending_orders(orders: &[Arc<Order>]) -> Vec<OrderUid> {
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
fn orders_with_balance(
    mut orders: Vec<Arc<Order>>,
    balances: &Balances,
    settlement_contract: Address,
    filter_bypass_orders: &HashSet<OrderUid>,
) -> (Vec<Arc<Order>>, Vec<OrderUid>) {
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
fn filter_dust_orders(
    mut orders: Vec<Arc<Order>>,
    balances: &Balances,
) -> (Vec<Arc<Order>>, Vec<OrderUid>) {
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

async fn get_orders_with_native_prices(
    orders: Vec<Arc<Order>>,
    native_price_estimator: &NativePriceUpdater,
    additional_tokens: impl IntoIterator<Item = Address>,
    timeout: Duration,
) -> (
    Vec<Arc<Order>>,
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
    orders: &[Arc<Order>],
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::{Address, B256},
        futures::FutureExt,
        maplit::{btreemap, hashset},
        model::order::{OrderBuilder, OrderData, OrderMetadata, OrderUid},
        shared::{
            bad_token::list_based::DenyListedTokens,
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
        },
    };

    #[tokio::test]
    async fn get_orders_with_native_prices_with_timeout() {
        let token1 = Address::repeat_byte(1);
        let token2 = Address::repeat_byte(2);
        let token3 = Address::repeat_byte(3);

        let orders = vec![
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

        let (filtered_orders, _removed, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            vec![],
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(filtered_orders, [orders[1].clone()]);
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

        let orders = vec![
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
        let (alive_orders, _removed_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
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
        let (alive_orders, _removed_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            vec![token5],
            Duration::ZERO,
        )
        .await;

        assert_eq!(alive_orders, [orders[2].clone()]);
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

        let orders = vec![
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

        let (alive_orders, _removed_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            vec![],
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(alive_orders, orders);
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

        let banned_user_orders = find_banned_user_orders(
            &orders,
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
        let orders = vec![
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

        let pending_orders = find_presignature_pending_orders(&orders);
        assert_eq!(pending_orders, vec![presign_uid]);
    }

    #[test]
    fn filter_unsupported_tokens_() {
        let token0 = Address::with_last_byte(0);
        let token1 = Address::with_last_byte(1);
        let token2 = Address::with_last_byte(2);
        let deny_listed_tokens = DenyListedTokens::new(vec![token0]);
        let orders = vec![
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
        let unsupported_tokens_orders = find_unsupported_tokens(&orders, &deny_listed_tokens);
        assert_eq!(
            unsupported_tokens_orders,
            [orders[0].metadata.uid, orders[2].metadata.uid]
        );
    }

    #[test]
    fn orders_with_balance_() {
        let settlement_contract = Address::repeat_byte(1);
        let orders = vec![
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
        let (alive_orders, _removed_orders) =
            orders_with_balance(orders.clone(), &balances, settlement_contract, &no_bypass);
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

        let orders = vec![
            regular_order.clone(),
            eip1271_order.clone(),
            wrapper_order.clone(),
        ];
        let balances: Balances = Default::default(); // No balances

        // EIP-1271 order and wrapper order should be retained, regular order filtered
        let wrapper_set = HashSet::from([wrapper_order_uid]);
        let (alive_orders, _removed_orders) =
            orders_with_balance(orders.clone(), &balances, settlement_contract, &wrapper_set);
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
        let (alive_orders, _removed_orders) =
            orders_with_balance(orders, &balances, settlement_contract, &empty_set);
        assert_eq!(alive_orders.len(), 1);
        assert_eq!(alive_orders[0].metadata.uid, eip1271_order.metadata.uid);
    }
}
