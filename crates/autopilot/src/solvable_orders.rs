use {
    crate::{
        boundary::{self, SolvableOrders},
        domain::{self, auction::Price, eth},
        infra::{self, banned},
    },
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    database::order_events::OrderEventLabel,
    futures::{FutureExt, future::join_all},
    indexmap::IndexSet,
    itertools::{Either, Itertools},
    model::{
        order::{Order, OrderClass, OrderUid},
        signature::Signature,
        time::now_in_epoch_seconds,
    },
    number::conversions::u256_to_big_decimal,
    primitive_types::{H160, H256, U256},
    prometheus::{Histogram, HistogramVec, IntCounter, IntCounterVec, IntGauge, IntGaugeVec},
    shared::{
        account_balances::{BalanceFetching, Query},
        bad_token::BadTokenDetecting,
        price_estimation::{
            native::{NativePriceEstimating, to_normalized_price},
            native_price_cache::CachingNativePriceEstimator,
        },
        remaining_amounts,
        signature_validator::{SignatureCheck, SignatureValidating},
    },
    std::{
        collections::{BTreeMap, HashMap, HashSet, btree_map::Entry},
        future::Future,
        sync::Arc,
        time::Duration,
    },
    strum::VariantNames,
    tokio::{sync::Mutex, time::Instant},
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
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    cache: Mutex<Option<Inner>>,
    native_price_estimator: Arc<CachingNativePriceEstimator>,
    signature_validator: Arc<dyn SignatureValidating>,
    metrics: &'static Metrics,
    weth: H160,
    limit_order_price_factor: BigDecimal,
    protocol_fees: domain::ProtocolFees,
    cow_amm_registry: cow_amm::Registry,
    native_price_timeout: Duration,
    settlement_contract: H160,
}

type Balances = HashMap<Query, U256>;

struct Inner {
    auction: domain::RawAuctionData,
    solvable_orders: boundary::SolvableOrders,
}

impl SolvableOrdersCache {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        min_order_validity_period: Duration,
        persistence: infra::Persistence,
        banned_users: banned::Users,
        balance_fetcher: Arc<dyn BalanceFetching>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        native_price_estimator: Arc<CachingNativePriceEstimator>,
        signature_validator: Arc<dyn SignatureValidating>,
        weth: H160,
        limit_order_price_factor: BigDecimal,
        protocol_fees: domain::ProtocolFees,
        cow_amm_registry: cow_amm::Registry,
        native_price_timeout: Duration,
        settlement_contract: H160,
    ) -> Arc<Self> {
        let self_ = Arc::new(Self {
            min_order_validity_period,
            persistence,
            banned_users,
            balance_fetcher,
            bad_token_detector,
            cache: Mutex::new(None),
            native_price_estimator,
            signature_validator,
            metrics: Metrics::instance(observe::metrics::get_storage_registry()).unwrap(),
            weth,
            limit_order_price_factor,
            protocol_fees,
            cow_amm_registry,
            native_price_timeout,
            settlement_contract,
        });
        self_
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
    pub async fn update(&self, block: u64) -> Result<()> {
        let start = Instant::now();

        let db_solvable_orders = self.get_solvable_orders().await?;

        let orders = db_solvable_orders
            .orders
            .values()
            .cloned()
            .collect::<Vec<_>>();

        let mut counter = OrderFilterCounter::new(self.metrics, &orders);
        let mut invalid_order_uids = HashSet::new();
        let mut filtered_order_events = Vec::new();

        let (balances, orders, cow_amms) = {
            let queries = orders.iter().map(Query::from_order).collect::<Vec<_>>();
            tokio::join!(
                self.fetch_balances(queries),
                self.filter_invalid_orders(orders, &mut counter, &mut invalid_order_uids,),
                self.timed_future("cow_amm_registry", self.cow_amm_registry.amms()),
            )
        };

        let orders = orders_with_balance(orders, &balances, self.settlement_contract);
        let removed = counter.checkpoint("insufficient_balance", &orders);
        invalid_order_uids.extend(removed);

        let orders = filter_dust_orders(orders, &balances);
        let removed = counter.checkpoint("dust_order", &orders);
        filtered_order_events.extend(removed);

        let cow_amm_tokens = cow_amms
            .iter()
            .flat_map(|cow_amm| cow_amm.traded_tokens())
            .cloned()
            .collect::<Vec<_>>();

        // create auction
        let (orders, mut prices) = self
            .timed_future(
                "get_orders_with_native_prices",
                get_orders_with_native_prices(
                    orders,
                    &self.native_price_estimator,
                    self.metrics,
                    cow_amm_tokens,
                    self.native_price_timeout,
                ),
            )
            .await;
        // Add WETH price if it's not already there to support ETH wrap when required.
        if let Entry::Vacant(entry) = prices.entry(self.weth) {
            let weth_price = self
                .timed_future(
                    "weth_price_fetch",
                    self.native_price_estimator.estimate_native_price(self.weth),
                )
                .await
                .expect("weth price fetching can never fail");
            let weth_price = to_normalized_price(weth_price)
                .expect("weth price can never be outside of U256 range");

            entry.insert(weth_price);
        }

        let removed = counter.checkpoint("missing_price", &orders);
        filtered_order_events.extend(removed);

        let orders = filter_mispriced_limit_orders(orders, &prices, &self.limit_order_price_factor);
        let removed = counter.checkpoint("out_of_market", &orders);
        filtered_order_events.extend(removed);

        let removed = counter.record(&orders);
        filtered_order_events.extend(removed);

        // spawning a background task since `order_events` table insert operation takes
        // a while and the result is ignored.
        self.persistence.store_order_events(
            invalid_order_uids.iter().map(|id| domain::OrderUid(id.0)),
            OrderEventLabel::Invalid,
        );
        self.persistence.store_order_events(
            filtered_order_events
                .iter()
                .map(|id| domain::OrderUid(id.0)),
            OrderEventLabel::Filtered,
        );

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
            .map(|cow_amm| cow_amm.address())
            .cloned()
            .map(eth::Address::from)
            .collect::<Vec<_>>();
        let auction = domain::RawAuctionData {
            block,
            orders: orders
                .into_iter()
                .map(|order| {
                    let quote = db_solvable_orders
                        .quotes
                        .get(&order.metadata.uid.into())
                        .cloned();
                    self.protocol_fees
                        .apply(order, quote, &surplus_capturing_jit_order_owners)
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
        self.metrics
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
        mut orders: Vec<Order>,
        counter: &mut OrderFilterCounter,
        invalid_order_uids: &mut HashSet<OrderUid>,
    ) -> Vec<Order> {
        let (banned_user_orders, invalid_signature_orders, unsupported_token_orders) = tokio::join!(
            self.timed_future(
                "banned_user_filtering",
                find_banned_user_orders(&orders, &self.banned_users)
            ),
            self.timed_future(
                "invalid_signature_filtering",
                find_invalid_signature_orders(&orders, self.signature_validator.as_ref())
            ),
            self.timed_future(
                "unsupported_token_filtering",
                find_unsupported_tokens(&orders, self.bad_token_detector.clone())
            ),
        );

        counter.checkpoint_by_invalid_orders("banned_user", &banned_user_orders);
        counter.checkpoint_by_invalid_orders("invalid_signature", &invalid_signature_orders);
        counter.checkpoint_by_invalid_orders("unsupported_token", &unsupported_token_orders);
        invalid_order_uids.extend(banned_user_orders);
        invalid_order_uids.extend(invalid_signature_orders);
        invalid_order_uids.extend(unsupported_token_orders);

        orders.retain(|order| !invalid_order_uids.contains(&order.metadata.uid));
        orders
    }

    pub fn track_auction_update(&self, result: &str) {
        self.metrics
            .auction_update
            .with_label_values(&[result])
            .inc();
    }

    /// Runs the future and collects runtime metrics.
    async fn timed_future<T>(&self, label: &str, fut: impl Future<Output = T>) -> T {
        let _timer = self
            .metrics
            .auction_update_stage_time
            .with_label_values(&[label])
            .start_timer();
        fut.await
    }
}

/// Finds all orders whose owners or receivers are in the set of "banned"
/// users.
async fn find_banned_user_orders(orders: &[Order], banned_users: &banned::Users) -> Vec<OrderUid> {
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
            std::iter::once(&order.metadata.owner)
                .chain(&order.data.receiver)
                .any(|addr| banned.contains(addr))
                .then_some(order.metadata.uid)
        })
        .collect()
}

async fn get_native_prices(
    tokens: &[H160],
    native_price_estimator: &CachingNativePriceEstimator,
    timeout: Duration,
) -> BTreeMap<H160, U256> {
    native_price_estimator
        .estimate_native_prices_with_timeout(tokens, timeout)
        .await
        .into_iter()
        .flat_map(|(token, result)| {
            let price = to_normalized_price(result.ok()?)?;
            Some((token, price))
        })
        .collect()
}

/// Finds unsigned PreSign and EIP-1271 orders whose signatures are no longer
/// validating.
async fn find_invalid_signature_orders(
    orders: &[Order],
    signature_validator: &dyn SignatureValidating,
) -> Vec<OrderUid> {
    let (mut invalid_orders, orders): (Vec<_>, Vec<_>) = orders.iter().partition_map(|order| {
        if matches!(
            order.metadata.status,
            model::order::OrderStatus::PresignaturePending
        ) {
            Either::Left(order.metadata.uid)
        } else {
            Either::Right(order)
        }
    });

    let checks = orders
        .iter()
        .filter_map(|order| match &order.signature {
            Signature::Eip1271(signature) => {
                let (H256(hash), signer, _) = order.metadata.uid.parts();
                Some(SignatureCheck {
                    signer,
                    hash,
                    signature: signature.clone(),
                    interactions: order.interactions.pre.clone(),
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if checks.is_empty() {
        return invalid_orders;
    }

    let mut validations = signature_validator
        .validate_signatures(checks)
        .await
        .into_iter();
    for order in orders {
        if !matches!(&order.signature, Signature::Eip1271(_)) {
            continue;
        }
        if let Err(err) = validations.next().unwrap() {
            tracing::warn!(
                order =% order.metadata.uid, ?err,
                "invalid EIP-1271 signature"
            );
            invalid_orders.push(order.metadata.uid)
        }
    }

    invalid_orders
}

/// Removes orders that can't possibly be settled because there isn't enough
/// balance.
fn orders_with_balance(
    mut orders: Vec<Order>,
    balances: &Balances,
    settlement_contract: H160,
) -> Vec<Order> {
    // Prefer newer orders over older ones.
    orders.sort_by_key(|order| std::cmp::Reverse(order.metadata.creation_date));
    orders.retain(|order| {
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

        if order.data.partially_fillable && balance >= 1.into() {
            return true;
        }

        let needed_balance = match order.data.sell_amount.checked_add(order.data.fee_amount) {
            None => return false,
            Some(balance) => balance,
        };
        balance >= needed_balance
    });
    orders
}

/// Filters out dust orders i.e. partially fillable orders that, when scaled
/// have a 0 buy or sell amount.
fn filter_dust_orders(mut orders: Vec<Order>, balances: &Balances) -> Vec<Order> {
    orders.retain(|order| {
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
    });
    orders
}

async fn get_orders_with_native_prices(
    orders: Vec<Order>,
    native_price_estimator: &CachingNativePriceEstimator,
    metrics: &Metrics,
    additional_tokens: impl IntoIterator<Item = H160>,
    timeout: Duration,
) -> (Vec<Order>, BTreeMap<H160, U256>) {
    let traded_tokens = orders
        .iter()
        .flat_map(|order| [order.data.sell_token, order.data.buy_token])
        .chain(additional_tokens)
        .collect::<HashSet<_>>();

    let prices = get_native_prices(
        &traded_tokens.into_iter().collect::<Vec<_>>(),
        native_price_estimator,
        timeout,
    )
    .await;

    // Filter orders so that we only return orders that have prices
    let mut filtered_market_orders = 0_i64;
    let (usable, filtered): (Vec<_>, Vec<_>) = orders.into_iter().partition(|order| {
        let (t0, t1) = (&order.data.sell_token, &order.data.buy_token);
        match (prices.get(t0), prices.get(t1)) {
            (Some(_), Some(_)) => true,
            _ => {
                filtered_market_orders += i64::from(order.metadata.class == OrderClass::Market);
                false
            }
        }
    });
    let tokens_by_priority = prioritize_missing_prices(filtered);
    native_price_estimator.replace_high_priority(tokens_by_priority);

    // Record separate metrics just for missing native token prices for market
    // orders, as they should be prioritized.
    metrics
        .auction_market_order_missing_price
        .set(filtered_market_orders);

    (usable, prices)
}

/// Computes which missing native prices are the most urgent to fetch.
/// Prices for recent orders have the highest priority because those are most
/// likely market orders which users expect to get settled ASAP.
/// For the remaining orders we prioritize token prices that are needed the most
/// often. That way we have the chance to make a majority of orders solvable
/// with very few fetch requests.
fn prioritize_missing_prices(mut orders: Vec<Order>) -> IndexSet<H160> {
    /// How old an order can be at most to be considered a market order.
    const MARKET_ORDER_AGE_MINUTES: i64 = 30;
    let market_order_age = chrono::Duration::minutes(MARKET_ORDER_AGE_MINUTES);
    let now = chrono::Utc::now();

    // newer orders at the start
    orders.sort_by_key(|o| std::cmp::Reverse(o.metadata.creation_date));

    let mut high_priority_tokens = IndexSet::new();
    let mut most_used_tokens = HashMap::<H160, usize>::new();
    for order in orders {
        let sell_token = order.data.sell_token;
        let buy_token = order.data.buy_token;
        let is_market = now.signed_duration_since(order.metadata.creation_date) <= market_order_age;

        if is_market {
            // already correct priority because orders were sorted by creation_date
            high_priority_tokens.extend([sell_token, buy_token]);
        } else {
            // count how often tokens are used to prioritize popular tokens
            *most_used_tokens.entry(sell_token).or_default() += 1;
            *most_used_tokens.entry(buy_token).or_default() += 1;
        }
    }

    // popular tokens at the start
    let most_used_tokens = most_used_tokens
        .into_iter()
        .sorted_by_key(|entry| std::cmp::Reverse(entry.1))
        .map(|(token, _)| token);

    high_priority_tokens.extend(most_used_tokens);
    high_priority_tokens
}

async fn find_unsupported_tokens(
    orders: &[Order],
    bad_token: Arc<dyn BadTokenDetecting>,
) -> Vec<OrderUid> {
    let bad_tokens = join_all(
        orders
            .iter()
            .flat_map(|o| o.data.token_pair().into_iter().flatten())
            .unique()
            .map(|token| {
                let bad_token = bad_token.clone();
                async move {
                    match bad_token.detect(token).await {
                        Ok(quality) => (!quality.is_good()).then_some(token),
                        Err(err) => {
                            tracing::warn!(
                                ?token,
                                ?err,
                                "unable to determine token quality, assume good"
                            );
                            Some(token)
                        }
                    }
                }
            }),
    )
    .await
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>();

    orders
        .iter()
        .filter_map(|order| {
            order
                .data
                .token_pair()
                .into_iter()
                .flatten()
                .any(|token| bad_tokens.contains(&token))
                .then_some(order.metadata.uid)
        })
        .collect()
}

/// Filter out limit orders which are far enough outside the estimated native
/// token price.
fn filter_mispriced_limit_orders(
    mut orders: Vec<Order>,
    prices: &BTreeMap<H160, U256>,
    price_factor: &BigDecimal,
) -> Vec<Order> {
    orders.retain(|order| {
        if !order.is_limit_order() {
            return true;
        }

        let sell_price = *prices.get(&order.data.sell_token).unwrap();
        let buy_price = *prices.get(&order.data.buy_token).unwrap();

        // Convert the sell and buy price to the native token (ETH) and make sure that
        // sell is higher than buy with the configurable price factor.
        let (sell_native, buy_native) = match (
            order.data.sell_amount.checked_mul(sell_price),
            order.data.buy_amount.checked_mul(buy_price),
        ) {
            (Some(sell), Some(buy)) => (sell, buy),
            _ => {
                tracing::warn!(
                    order_uid = %order.metadata.uid,
                    "limit order overflow computing native amounts",
                );
                return false;
            }
        };

        let sell_native = u256_to_big_decimal(&sell_native);
        let buy_native = u256_to_big_decimal(&buy_native);

        sell_native >= buy_native * price_factor
    });
    orders
}

/// Order filtering state for recording filtered orders over the course of
/// building an auction.
struct OrderFilterCounter {
    metrics: &'static Metrics,

    /// Mapping of remaining order UIDs to their classes.
    orders: HashMap<OrderUid, OrderClass>,
    /// Running tally for counts of filtered orders.
    counts: HashMap<Reason, usize>,
}

type Reason = &'static str;

impl OrderFilterCounter {
    fn new(metrics: &'static Metrics, orders: &[Order]) -> Self {
        // Eagerly store the candidate orders. This ensures that that gauge is
        // always up to date even if there are errors in the auction building
        // process.
        let initial_counts = orders
            .iter()
            .counts_by(|order| order.metadata.class.as_ref());
        for class in OrderClass::VARIANTS {
            let count = initial_counts.get(class).copied().unwrap_or_default();
            metrics
                .auction_candidate_orders
                .with_label_values(&[class])
                .set(i64::try_from(count).unwrap_or(i64::MAX));
        }

        Self {
            metrics,
            orders: orders
                .iter()
                .map(|order| (order.metadata.uid, order.metadata.class))
                .collect(),
            counts: HashMap::new(),
        }
    }

    /// Creates a new checkpoint from the current remaining orders.
    fn checkpoint(&mut self, reason: Reason, orders: &[Order]) -> Vec<OrderUid> {
        let filtered_orders = orders
            .iter()
            .fold(self.orders.clone(), |mut order_uids, order| {
                order_uids.remove(&order.metadata.uid);
                order_uids
            });

        *self.counts.entry(reason).or_default() += filtered_orders.len();
        for order_uid in filtered_orders.keys() {
            self.orders.remove(order_uid).unwrap();
        }
        if !filtered_orders.is_empty() {
            tracing::debug!(
                %reason,
                count = filtered_orders.len(),
                orders = ?filtered_orders, "filtered orders"
            );
        }
        filtered_orders.into_keys().collect()
    }

    /// Creates a new checkpoint based on the found invalid orders.
    fn checkpoint_by_invalid_orders(&mut self, reason: Reason, invalid_orders: &[OrderUid]) {
        if invalid_orders.is_empty() {
            return;
        }

        let mut counter = 0;
        for order_uid in invalid_orders {
            if self.orders.remove(order_uid).is_some() {
                counter += 1;
            }
        }
        *self.counts.entry(reason).or_default() += counter;
        if counter > 0 {
            tracing::debug!(
                %reason,
                count = invalid_orders.len(),
                orders = ?invalid_orders, "filtered orders"
            );
        }
    }

    /// Records the filter counter to metrics.
    /// If there are orders that have been filtered out since the last
    /// checkpoint these orders will get recorded with the readon "other".
    /// Returns these catch-all orders.
    fn record(mut self, orders: &[Order]) -> Vec<OrderUid> {
        let removed = self.checkpoint("other", orders);

        self.metrics.auction_creations.inc();

        let remaining_counts = self.orders.iter().counts_by(|(_, class)| class.as_ref());
        for class in OrderClass::VARIANTS {
            let count = remaining_counts.get(class).copied().unwrap_or_default();
            self.metrics
                .auction_solvable_orders
                .with_label_values(&[class])
                .set(i64::try_from(count).unwrap_or(i64::MAX));
        }

        for (reason, count) in self.counts {
            self.metrics
                .auction_filtered_orders
                .with_label_values(&[reason])
                .set(i64::try_from(count).unwrap_or(i64::MAX));
        }

        removed
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        futures::FutureExt,
        maplit::{btreemap, hashset},
        mockall::predicate::eq,
        model::{
            interaction::InteractionData,
            order::{Interactions, OrderBuilder, OrderData, OrderMetadata, OrderUid},
        },
        primitive_types::H160,
        shared::{
            bad_token::list_based::ListBasedDetector,
            price_estimation::{PriceEstimationError, native::MockNativePriceEstimating},
            signature_validator::{MockSignatureValidating, SignatureValidationError},
        },
    };

    #[tokio::test]
    async fn get_orders_with_native_prices_with_timeout() {
        let token1 = H160([1; 20]);
        let token2 = H160([2; 20]);
        let token3 = H160([3; 20]);

        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token3)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
        ];

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf(move |token| *token == token1)
            .returning(|_| async { Ok(2.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token2)
            .returning(|_| async { Err(PriceEstimationError::NoLiquidity) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token3)
            .returning(|_| async { Ok(0.25) }.boxed());

        let native_price_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            Duration::from_secs(10),
            Duration::MAX,
            None,
            Default::default(),
            3,
            Default::default(),
        );
        let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();

        let (filtered_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            metrics,
            vec![],
            Duration::from_millis(100),
        )
        .await;
        assert_eq!(filtered_orders, [orders[1].clone()]);
        assert_eq!(
            prices,
            btreemap! {
                token1 => U256::from(2_000_000_000_000_000_000_u128),
                token3 => U256::from(250_000_000_000_000_000_u128),
            }
        );
    }

    #[tokio::test]
    async fn filters_orders_with_tokens_without_native_prices() {
        let token1 = H160([1; 20]);
        let token2 = H160([2; 20]);
        let token3 = H160([3; 20]);
        let token4 = H160([4; 20]);
        let token5 = H160([5; 20]);

        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
            OrderBuilder::default()
                .with_sell_token(token2)
                .with_buy_token(token3)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token3)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
            OrderBuilder::default()
                .with_sell_token(token2)
                .with_buy_token(token4)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
        ];

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .withf(move |token| *token == token1)
            .returning(|_| async { Ok(2.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token2)
            .returning(|_| async { Err(PriceEstimationError::NoLiquidity) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token3)
            .returning(|_| async { Ok(0.25) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token4)
            .returning(|_| async { Ok(0.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token5)
            .returning(|_| async { Ok(5.) }.boxed());

        let native_price_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            Duration::from_secs(10),
            Duration::MAX,
            None,
            Default::default(),
            1,
            Default::default(),
        );
        let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();

        // We'll have no native prices in this call. But this call will cause a
        // background task to fetch the missing prices so we'll have them in the
        // next call.
        let (filtered_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            metrics,
            vec![token5],
            Duration::ZERO,
        )
        .await;
        assert!(filtered_orders.is_empty());
        assert!(prices.is_empty());

        // Wait for native prices to get fetched.
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Now we have all the native prices we want.
        let (filtered_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            metrics,
            vec![token5],
            Duration::ZERO,
        )
        .await;

        assert_eq!(filtered_orders, [orders[2].clone()]);
        assert_eq!(
            prices,
            btreemap! {
                token1 => U256::from(2_000_000_000_000_000_000_u128),
                token3 => U256::from(250_000_000_000_000_000_u128),
                token5 => U256::from(5_000_000_000_000_000_000_u128),
            }
        );
    }

    #[tokio::test]
    async fn check_native_price_approximations() {
        let token1 = H160([1; 20]);
        let token2 = H160([2; 20]);
        let token3 = H160([3; 20]);

        let token_approx1 = H160([4; 20]);
        let token_approx2 = H160([5; 20]);

        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token3)
                .with_buy_amount(1.into())
                .with_sell_amount(1.into())
                .build(),
        ];

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token3)
            .returning(|_| async { Ok(3.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token_approx1)
            .returning(|_| async { Ok(40.) }.boxed());
        native_price_estimator
            .expect_estimate_native_price()
            .times(1)
            .withf(move |token| *token == token_approx2)
            .returning(|_| async { Ok(50.) }.boxed());

        let native_price_estimator = CachingNativePriceEstimator::new(
            Box::new(native_price_estimator),
            Duration::from_secs(10),
            Duration::MAX,
            None,
            Default::default(),
            3,
            // Set to use native price approximations for the following tokens
            HashMap::from([(token1, token_approx1), (token2, token_approx2)]),
        );
        let metrics = Metrics::instance(observe::metrics::get_storage_registry()).unwrap();

        let (filtered_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            metrics,
            vec![],
            Duration::from_secs(10),
        )
        .await;
        assert_eq!(filtered_orders, orders);
        assert_eq!(
            prices,
            btreemap! {
                token1 => U256::from(40_000_000_000_000_000_000_u128),
                token2 => U256::from(50_000_000_000_000_000_000_u128),
                token3 => U256::from(3_000_000_000_000_000_000_u128),
            }
        );
    }

    #[tokio::test]
    async fn filters_banned_users() {
        let banned_users = hashset!(H160([0xba; 20]), H160([0xbb; 20]));
        let orders = [
            H160([1; 20]),
            H160([1; 20]),
            H160([0xba; 20]),
            H160([2; 20]),
            H160([0xba; 20]),
            H160([0xbb; 20]),
            H160([3; 20]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, owner)| Order {
            metadata: OrderMetadata {
                owner,
                uid: OrderUid([i as u8; 56]),
                ..Default::default()
            },
            data: OrderData {
                buy_amount: 1.into(),
                sell_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
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

    #[tokio::test]
    async fn filters_invalidated_eip1271_signatures() {
        let orders = vec![
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([1; 32]), H160([11; 20]), 1),
                    ..Default::default()
                },
                interactions: Interactions {
                    pre: vec![InteractionData {
                        target: H160([0xe1; 20]),
                        value: U256::zero(),
                        call_data: vec![1, 2],
                    }],
                    post: vec![InteractionData {
                        target: H160([0xe2; 20]),
                        value: U256::zero(),
                        call_data: vec![3, 4],
                    }],
                },
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([2; 32]), H160([22; 20]), 2),
                    ..Default::default()
                },
                signature: Signature::Eip1271(vec![2, 2]),
                interactions: Interactions {
                    pre: vec![InteractionData {
                        target: H160([0xe3; 20]),
                        value: U256::zero(),
                        call_data: vec![5, 6],
                    }],
                    post: vec![InteractionData {
                        target: H160([0xe4; 20]),
                        value: U256::zero(),
                        call_data: vec![7, 9],
                    }],
                },
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([3; 32]), H160([33; 20]), 3),
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([4; 32]), H160([44; 20]), 4),
                    ..Default::default()
                },
                signature: Signature::Eip1271(vec![4, 4, 4, 4]),
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([5; 32]), H160([55; 20]), 5),
                    ..Default::default()
                },
                signature: Signature::Eip1271(vec![5, 5, 5, 5, 5]),
                ..Default::default()
            },
        ];

        let mut signature_validator = MockSignatureValidating::new();
        signature_validator
            .expect_validate_signatures()
            .with(eq(vec![
                SignatureCheck {
                    signer: H160([22; 20]),
                    hash: [2; 32],
                    signature: vec![2, 2],
                    interactions: vec![InteractionData {
                        target: H160([0xe3; 20]),
                        value: U256::zero(),
                        call_data: vec![5, 6],
                    }],
                },
                SignatureCheck {
                    signer: H160([44; 20]),
                    hash: [4; 32],
                    signature: vec![4, 4, 4, 4],
                    interactions: vec![],
                },
                SignatureCheck {
                    signer: H160([55; 20]),
                    hash: [5; 32],
                    signature: vec![5, 5, 5, 5, 5],
                    interactions: vec![],
                },
            ]))
            .returning(|_| vec![Ok(()), Err(SignatureValidationError::Invalid), Ok(())]);

        let invalid_signature_orders =
            find_invalid_signature_orders(&orders, &signature_validator).await;
        assert_eq!(
            invalid_signature_orders,
            vec![OrderUid::from_parts(H256([4; 32]), H160([44; 20]), 4)]
        );
    }

    #[test]
    fn filter_unsupported_tokens_() {
        let token0 = H160::from_low_u64_le(0);
        let token1 = H160::from_low_u64_le(1);
        let token2 = H160::from_low_u64_le(2);
        let bad_token = Arc::new(ListBasedDetector::deny_list(vec![token0]));
        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(token0)
                .with_buy_token(token1)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token1)
                .with_buy_token(token2)
                .build(),
            OrderBuilder::default()
                .with_sell_token(token0)
                .with_buy_token(token2)
                .build(),
        ];
        let unsupported_tokens_orders = find_unsupported_tokens(&orders, bad_token)
            .now_or_never()
            .unwrap();
        assert_eq!(
            unsupported_tokens_orders,
            [orders[0].metadata.uid, orders[2].metadata.uid]
        );
    }

    #[test]
    fn filters_mispriced_orders() {
        let sell_token = H160([1; 20]);
        let buy_token = H160([2; 20]);

        // Prices are set such that 1 sell token is equivalent to 2 buy tokens.
        // Additionally, they are scaled to large values to allow for overflows.
        let prices = btreemap! {
            sell_token => U256::MAX / 100,
            buy_token => U256::MAX / 200,
        };
        let price_factor = "0.95".parse().unwrap();

        let order = |sell_amount: u8, buy_amount: u8| Order {
            data: OrderData {
                sell_token,
                sell_amount: sell_amount.into(),
                buy_token,
                buy_amount: buy_amount.into(),
                ..Default::default()
            },
            metadata: OrderMetadata {
                class: OrderClass::Limit,
                ..Default::default()
            },
            ..Default::default()
        };

        let valid_orders = vec![
            // Reasonably priced order, doesn't get filtered.
            order(100, 200),
            // Slightly out of price order, doesn't get filtered.
            order(10, 21),
        ];

        let invalid_orders = vec![
            // Out of price order gets filtered out.
            order(10, 100),
            // Overflow sell value gets filtered.
            order(255, 1),
            // Overflow buy value gets filtered.
            order(100, 255),
        ];

        let orders = [valid_orders.clone(), invalid_orders].concat();
        assert_eq!(
            filter_mispriced_limit_orders(orders, &prices, &price_factor),
            valid_orders,
        );

        let mut order = order(10, 21);
        order.data.partially_fillable = true;
        let orders = vec![order];
        assert_eq!(
            filter_mispriced_limit_orders(orders, &prices, &price_factor).len(),
            1
        );
    }

    #[test]
    fn orders_with_balance_() {
        let settlement_contract = H160([1; 20]);
        let orders = vec![
            // enough balance for sell and fee
            Order {
                data: OrderData {
                    sell_token: H160::from_low_u64_be(2),
                    sell_amount: 1.into(),
                    fee_amount: 1.into(),
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            // missing fee balance
            Order {
                data: OrderData {
                    sell_token: H160::from_low_u64_be(3),
                    sell_amount: 1.into(),
                    fee_amount: 1.into(),
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            },
            // at least 1 partially fillable balance
            Order {
                data: OrderData {
                    sell_token: H160::from_low_u64_be(4),
                    sell_amount: 2.into(),
                    fee_amount: 0.into(),
                    partially_fillable: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            // 0 partially fillable balance
            Order {
                data: OrderData {
                    sell_token: H160::from_low_u64_be(5),
                    sell_amount: 2.into(),
                    fee_amount: 0.into(),
                    partially_fillable: true,
                    ..Default::default()
                },
                ..Default::default()
            },
            // considered flashloan order because of special receiver
            Order {
                data: OrderData {
                    sell_token: H160::from_low_u64_be(6),
                    sell_amount: 200.into(),
                    fee_amount: 0.into(),
                    partially_fillable: true,
                    receiver: Some(settlement_contract),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        let balances = [
            (Query::from_order(&orders[0]), 2.into()),
            (Query::from_order(&orders[1]), 1.into()),
            (Query::from_order(&orders[2]), 1.into()),
            (Query::from_order(&orders[3]), 0.into()),
            (Query::from_order(&orders[4]), 0.into()),
        ]
        .into_iter()
        .collect();
        let expected = &[0, 2, 4];

        let filtered = orders_with_balance(orders.clone(), &balances, settlement_contract);
        assert_eq!(filtered.len(), expected.len());
        for index in expected {
            let found = filtered.iter().any(|o| o.data == orders[*index].data);
            assert!(found, "{}", index);
        }
    }

    #[test]
    fn prioritizes_missing_prices() {
        let now = chrono::Utc::now();
        let token = H160::from_low_u64_be;

        let order = |sell_token, buy_token, age| Order {
            metadata: OrderMetadata {
                creation_date: now - chrono::Duration::minutes(age),
                ..Default::default()
            },
            data: OrderData {
                sell_token,
                buy_token,
                ..Default::default()
            },
            ..Default::default()
        };

        let orders = vec![
            order(token(4), token(6), 31),
            order(token(4), token(6), 31),
            order(token(1), token(2), 29), // older market order
            order(token(5), token(6), 31),
            order(token(1), token(3), 1), // youngest market order
        ];
        let result = prioritize_missing_prices(orders);
        assert!(result.into_iter().eq([
            token(1), // coming from youngest market order
            token(3), // coming from youngest market order
            token(2), // coming from older market order
            token(6), // coming from limit order (part of 3 orders)
            token(4), // coming from limit order (part of 2 orders)
            token(5), // coming from limit order (part of 1 orders)
        ]));
    }
}
