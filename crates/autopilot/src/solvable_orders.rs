use crate::{database::Postgres, risk_adjusted_rewards};
use anyhow::{Context as _, Result};
use bigdecimal::BigDecimal;
use chrono::Utc;
use futures::{FutureExt as _, StreamExt as _};
use itertools::Itertools;
use model::{
    auction::Auction,
    order::{Order, OrderClass},
    signature::Signature,
    time::now_in_epoch_seconds,
};
use number_conversions::u256_to_big_decimal;
use primitive_types::{H160, H256, U256};
use prometheus::{IntCounter, IntGaugeVec};
use shared::{
    account_balances::{BalanceFetching, Query},
    bad_token::BadTokenDetecting,
    current_block::CurrentBlockStream,
    price_estimation::native::NativePriceEstimating,
    signature_validator::{SignatureCheck, SignatureValidating},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    future::Future,
    iter::FromIterator,
    mem,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use strum::VariantNames;
use tokio::time::Instant;

// When creating the auction after solvable orders change we need to fetch native prices for a
// potentially large amount of tokens. This is the maximum amount of time we allot for this
// operation.
const MAX_AUCTION_CREATION_TIME: Duration = Duration::from_secs(10);

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// Auction creations.
    auction_creations: IntCounter,

    /// Auction solvable orders grouped by class.
    #[metric(labels("class"))]
    auction_solvable_orders: IntGaugeVec,

    /// Auction filtered orders grouped by class.
    #[metric(labels("class", "reason"))]
    auction_filtered_orders: IntGaugeVec,

    /// Auction errored price estimates.
    auction_errored_price_estimates: IntCounter,

    /// Auction price estimate timeouts.
    auction_price_estimate_timeouts: IntCounter,
}

/// Keeps track and updates the set of currently solvable orders.
/// For this we also need to keep track of user sell token balances for open orders so this is
/// retrievable as well.
/// The cache is updated in the background whenever a new block appears or when the cache is
/// explicitly notified that it should update for example because a new order got added to the order
/// book.
pub struct SolvableOrdersCache {
    min_order_validity_period: Duration,
    database: Postgres,
    banned_users: HashSet<H160>,
    balance_fetcher: Arc<dyn BalanceFetching>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    cache: Mutex<Inner>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
    signature_validator: Arc<dyn SignatureValidating>,
    metrics: &'static Metrics,
    // Optional because reward calculation only makes sense on mainnet. Other networks have 0 rewards.
    reward_calculator: Option<risk_adjusted_rewards::Calculator>,
    ethflow_contract_address: Option<H160>,
    surplus_fee_age: Duration,
    limit_order_price_factor: BigDecimal,
}

type Balances = HashMap<Query, U256>;

struct Inner {
    orders: SolvableOrders,
    balances: Balances,
}

#[derive(Clone, Debug)]
pub struct SolvableOrders {
    pub orders: Vec<Order>,
    pub update_time: Instant,
    pub latest_settlement_block: u64,
    pub block: u64,
}

impl SolvableOrdersCache {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        min_order_validity_period: Duration,
        database: Postgres,
        banned_users: HashSet<H160>,
        balance_fetcher: Arc<dyn BalanceFetching>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        current_block: CurrentBlockStream,
        native_price_estimator: Arc<dyn NativePriceEstimating>,
        signature_validator: Arc<dyn SignatureValidating>,
        update_interval: Duration,
        reward_calculator: Option<risk_adjusted_rewards::Calculator>,
        ethflow_contract_address: Option<H160>,
        surplus_fee_age: Duration,
        limit_order_price_factor: BigDecimal,
    ) -> Arc<Self> {
        let self_ = Arc::new(Self {
            min_order_validity_period,
            database,
            banned_users,
            balance_fetcher,
            bad_token_detector,
            cache: Mutex::new(Inner {
                orders: SolvableOrders {
                    orders: Default::default(),
                    update_time: Instant::now(),
                    latest_settlement_block: 0,
                    block: 0,
                },
                balances: Default::default(),
            }),
            native_price_estimator,
            signature_validator,
            metrics: Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
            reward_calculator,
            ethflow_contract_address,
            surplus_fee_age,
            limit_order_price_factor,
        });
        tokio::task::spawn(update_task(
            Arc::downgrade(&self_),
            update_interval,
            current_block,
        ));
        self_
    }

    /// Manually update solvable orders. Usually called by the background updating task.
    ///
    /// Usually this method is called from update_task. If it isn't, which is the case in unit tests,
    /// then concurrent calls might overwrite each other's results.
    pub async fn update(&self, block: u64) -> Result<()> {
        let min_valid_to = now_in_epoch_seconds() + self.min_order_validity_period.as_secs() as u32;
        let db_solvable_orders = self
            .database
            .solvable_orders(
                min_valid_to,
                Utc::now() - chrono::Duration::from_std(self.surplus_fee_age).unwrap(),
            )
            .await?;

        let mut orders = OrderFilter::new(self.metrics, db_solvable_orders.orders);

        filter_banned_user_orders(&mut orders, &self.banned_users);
        filter_unsupported_tokens(&mut orders, self.bad_token_detector.as_ref()).await?;
        filter_invalid_signature_orders(&mut orders, self.signature_validator.as_ref()).await;
        filter_limit_orders_with_insufficient_sell_amount(&mut orders);

        // If we update due to an explicit notification we can reuse existing balances as they
        // cannot have changed.
        let old_balances = {
            let inner = self.cache.lock().unwrap();
            if inner.orders.block == block {
                inner.balances.clone()
            } else {
                HashMap::new()
            }
        };
        let (mut new_balances, missing_queries) = new_balances(&old_balances, orders.as_slice());
        let fetched_balances = self.balance_fetcher.get_balances(&missing_queries).await;
        for (query, balance) in missing_queries.into_iter().zip(fetched_balances) {
            let balance = match balance {
                Ok(balance) => balance,
                Err(err) => {
                    tracing::warn!(
                        owner = %query.owner,
                        token = %query.token,
                        source = ?query.source,
                        error = ?err,
                        "failed to get balance"
                    );
                    continue;
                }
            };
            new_balances.insert(query, balance);
        }

        solvable_orders(&mut orders, &new_balances, self.ethflow_contract_address);
        for order in orders.as_slice_mut() {
            let query = Query::from_order(order);
            order.metadata.available_balance = new_balances.get(&query).copied();
        }

        // create auction
        let prices = filter_orders_with_native_prices(
            &mut orders,
            &*self.native_price_estimator,
            Instant::now() + MAX_AUCTION_CREATION_TIME,
            self.metrics,
        )
        .await;

        filter_mispriced_limit_orders(&mut orders, &prices, &self.limit_order_price_factor);

        let rewards = if let Some(calculator) = &self.reward_calculator {
            let rewards = calculator
                .calculate_many(orders.as_slice())
                .await
                .context("rewards")?;
            orders
                .as_slice()
                .iter()
                .zip(rewards)
                .filter_map(|(order, reward)| match reward {
                    Ok(reward) if reward > 0. => Some((order.metadata.uid, reward)),
                    Ok(_) => None,
                    Err(err) => {
                        tracing::warn!(
                            ?order.metadata.uid, ?err,
                            "error calculating risk adjusted reward"
                        );
                        None
                    }
                })
                .collect()
        } else {
            Default::default()
        };

        let orders = orders.finish();
        let auction = Auction {
            block,
            latest_settlement_block: db_solvable_orders.latest_settlement_block,
            orders: orders.clone(),
            prices,
            rewards,
        };

        let _id = self.database.replace_current_auction(&auction).await?;
        *self.cache.lock().unwrap() = Inner {
            orders: SolvableOrders {
                orders,
                update_time: Instant::now(),
                latest_settlement_block: db_solvable_orders.latest_settlement_block,
                block,
            },
            balances: new_balances,
        };

        tracing::debug!(
            "updated auction with {} solvable orders",
            auction.orders.len(),
        );

        Ok(())
    }

    pub fn last_update_time(&self) -> Instant {
        self.cache.lock().unwrap().orders.update_time
    }
}

/// Filters all orders whose owners are in the set of "banned" users.
fn filter_banned_user_orders(orders: &mut OrderFilter, banned_users: &HashSet<H160>) {
    orders.retain("banned_user", |order| {
        !banned_users.contains(&order.metadata.owner)
    });
}

/// Filters EIP-1271 orders whose signatures are no longer validating.
async fn filter_invalid_signature_orders(
    orders: &mut OrderFilter,
    signature_validator: &dyn SignatureValidating,
) {
    let checks = orders
        .as_slice()
        .iter()
        .filter_map(|order| match &order.signature {
            Signature::Eip1271(signature) => {
                let (H256(hash), signer, _) = order.metadata.uid.parts();
                Some(SignatureCheck {
                    signer,
                    hash,
                    signature: signature.clone(),
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if checks.is_empty() {
        return;
    }

    let mut validations = signature_validator
        .validate_signatures(checks)
        .await
        .into_iter();

    orders.filter("invalid_signature", |orders| {
        orders
            .into_iter()
            .filter(|order| {
                if let Signature::Eip1271(_) = &order.signature {
                    if let Err(err) = validations.next().unwrap() {
                        tracing::warn!(
                            order_uid =% order.metadata.uid, ?err,
                            "filtered order because of invalid EIP-1271 signature"
                        );
                        return false;
                    }
                }

                true
            })
            .collect()
    });
}

/// Returns existing balances and Vec of queries that need to be performed.
fn new_balances(old_balances: &Balances, orders: &[Order]) -> (HashMap<Query, U256>, Vec<Query>) {
    let mut new_balances = HashMap::new();
    let mut missing_queries = HashSet::new();
    for order in orders {
        let query = Query::from_order(order);
        match old_balances.get(&query) {
            Some(balance) => {
                new_balances.insert(query, *balance);
            }
            None => {
                missing_queries.insert(query);
            }
        }
    }
    let missing_queries = Vec::from_iter(missing_queries);
    (new_balances, missing_queries)
}

// The order book has to make a choice for which orders to include when a user has multiple orders
// selling the same token but not enough balance for all of them.
// Assumes balance fetcher is already tracking all balances.
fn solvable_orders(orders: &mut OrderFilter, balances: &Balances, ethflow_contract: Option<H160>) {
    orders.filter("insufficient_balance", |mut orders| {
        let mut orders_map = HashMap::<Query, Vec<Order>>::new();
        orders.sort_by_key(|order| std::cmp::Reverse(order.metadata.creation_date));
        for order in orders {
            let key = Query::from_order(&order);
            orders_map.entry(key).or_default().push(order);
        }

        let mut result = Vec::new();
        for (key, orders) in orders_map {
            let mut remaining_balance = match balances.get(&key) {
                Some(balance) => *balance,
                None => continue,
            };
            for order in orders {
                // For ethflow orders, there is no need to check the balance. The contract
                // ensures that there will always be sufficient balance, after the wrapAll
                // pre_interaction has been called.
                if Some(order.metadata.owner) == ethflow_contract {
                    result.push(order);
                    continue;
                }
                // TODO: This is overly pessimistic for partially filled orders where the needed balance
                // is lower. For partially fillable orders that cannot be fully filled because of the
                // balance we could also give them as much balance as possible instead of skipping. For
                // that we first need a way to communicate this to the solver. We could repurpose
                // availableBalance for this.
                let needed_balance = match max_transfer_out_amount(&order) {
                    // Should only ever happen if a partially fillable order has been filled completely
                    Ok(balance) if balance.is_zero() => continue,
                    Ok(balance) => balance,
                    Err(err) => {
                        // This should only happen if we read bogus order data from
                        // the database (either we allowed a bogus order to be
                        // created or we updated a good order incorrectly), so raise
                        // the alarm!
                        tracing::error!(
                            ?err,
                            ?order,
                            "error computing order max transfer out amount"
                        );
                        continue;
                    }
                };
                if let Some(balance) = remaining_balance.checked_sub(needed_balance) {
                    remaining_balance = balance;
                    result.push(order);
                } else {
                    tracing::debug!(
                        order_uid = ?order.metadata.uid,
                        "filtered order because of insufficient allowance/balance",
                    );
                }
            }
        }
        result
    });
}

/// Computes the maximum amount that can be transferred out for a given order.
///
/// While this is trivial for fill or kill orders (`sell_amount + fee_amount`),
/// partially fillable orders need to account for the already filled amount (so
/// a half-filled order would be `(sell_amount + fee_amount) / 2`).
///
/// Returns `Err` on overflow.
fn max_transfer_out_amount(order: &Order) -> Result<U256> {
    let remaining = shared::remaining_amounts::Remaining::from_order(order)?;
    let sell = remaining.remaining(order.data.sell_amount)?;
    let fee = remaining.remaining(order.data.fee_amount)?;
    sell.checked_add(fee).context("add")
}

/// Keep updating the cache every N seconds or when an update notification happens.
/// Exits when this becomes the only reference to the cache.
async fn update_task(
    cache: Weak<SolvableOrdersCache>,
    update_interval: Duration,
    current_block: CurrentBlockStream,
) {
    loop {
        // We are not updating on block changes because
        // - the state of orders could change even when the block does not like when an order
        //   gets cancelled off chain
        // - the event updater takes some time to run and if we go first we would not update the
        //   orders with the most recent events.
        tokio::time::sleep(update_interval).await;
        let cache = match cache.upgrade() {
            Some(self_) => self_,
            None => {
                tracing::debug!("exiting solvable orders update task");
                break;
            }
        };
        let block = current_block.borrow().number;
        let start = Instant::now();
        match cache.update(block).await {
            Ok(()) => tracing::debug!(
                %block,
                "updated solvable orders in {}s",
                start.elapsed().as_secs_f32()
            ),
            Err(err) => tracing::error!(
                ?err,
                %block,
                "failed to update solvable orders in {}s",
                start.elapsed().as_secs_f32()
            ),
        }
    }
}

async fn filter_orders_with_native_prices(
    orders: &mut OrderFilter,
    native_price_estimator: &dyn NativePriceEstimating,
    deadline: Instant,
    metrics: &Metrics,
) -> BTreeMap<H160, U256> {
    let traded_tokens = orders
        .as_slice()
        .iter()
        .flat_map(|order| [order.data.sell_token, order.data.buy_token])
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    let mut prices = HashMap::new();
    let mut price_stream = native_price_estimator.estimate_native_prices(&traded_tokens);
    let mut errored_estimates: u64 = 0;
    let collect_prices = async {
        while let Some((index, result)) = price_stream.next().await {
            let token = &traded_tokens[index];
            let price = match result {
                Ok(price) => price,
                Err(err) => {
                    errored_estimates += 1;
                    tracing::warn!(?token, ?err, "error estimating native token price");
                    continue;
                }
            };
            let price = match to_normalized_price(price) {
                Some(price) => price,
                None => continue,
            };
            prices.insert(*token, price);
        }
    };
    let timeout = match tokio::time::timeout_at(deadline, collect_prices).await {
        Ok(()) => false,
        Err(_) => {
            tracing::warn!(
                "auction native price collection took too long, got {} out of {}",
                prices.len(),
                traded_tokens.len()
            );
            true
        }
    };

    // Filter both orders and prices so that we only return orders that have prices and prices that
    // have orders.
    let mut used_prices = BTreeMap::new();
    orders.retain("missing_native_price", |order| {
        let (t0, t1) = (&order.data.sell_token, &order.data.buy_token);
        match (prices.get(t0), prices.get(t1)) {
            (Some(p0), Some(p1)) => {
                used_prices.insert(*t0, *p0);
                used_prices.insert(*t1, *p1);
                true
            }
            _ => {
                tracing::debug!(
                    order_uid = ?order.metadata.uid,
                    "filtered order because of missing native token price",
                );
                false
            }
        }
    });

    if timeout {
        metrics.auction_price_estimate_timeouts.inc();
    }
    metrics
        .auction_errored_price_estimates
        .inc_by(errored_estimates);

    used_prices
}

fn to_normalized_price(price: f64) -> Option<U256> {
    let uint_max = 2.0_f64.powi(256);

    let price_in_eth = 1e18 * price;
    if price_in_eth.is_normal() && price_in_eth >= 1. && price_in_eth < uint_max {
        Some(U256::from_f64_lossy(price_in_eth))
    } else {
        None
    }
}

async fn filter_unsupported_tokens(
    orders: &mut OrderFilter,
    bad_token: &dyn BadTokenDetecting,
) -> Result<()> {
    orders
        .try_retain_async("unsupported_token", |order| {
            let token_pair = order.data.token_pair().unwrap();
            async move {
                for token in token_pair {
                    if !bad_token.detect(token).await?.is_good() {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        })
        .await
}

fn filter_limit_orders_with_insufficient_sell_amount(orders: &mut OrderFilter) {
    orders.retain("insufficient_sell", |order| match &order.metadata.class {
        OrderClass::Limit(limit) => order.data.sell_amount > limit.surplus_fee,
        _ => true,
    });
}

/// Filter out limit orders which are far enough outside the estimated native token price.
fn filter_mispriced_limit_orders(
    orders: &mut OrderFilter,
    prices: &BTreeMap<H160, U256>,
    price_factor: &BigDecimal,
) {
    orders.retain("mispriced", |order| {
        let surplus_fee = match &order.metadata.class {
            OrderClass::Limit(limit) => limit.surplus_fee,
            _ => return true,
        };

        let effective_sell_amount = order.data.sell_amount.saturating_sub(surplus_fee);
        if effective_sell_amount.is_zero() {
            return false;
        }

        let sell_price = *prices.get(&order.data.sell_token).unwrap();
        let buy_price = *prices.get(&order.data.buy_token).unwrap();

        // Convert the sell and buy price to the native token (ETH) and make sure that sell
        // discounting the surplus fee is higher than buy with the configurable price factor.
        let (sell_native, buy_native) = match (
            effective_sell_amount.checked_mul(sell_price),
            order.data.buy_amount.checked_mul(buy_price),
        ) {
            (Some(sell), Some(buy)) => (sell, buy),
            _ => {
                tracing::debug!(
                    order_uid = %order.metadata.uid,
                    "limit order overflow computing native amounts; skipping",
                );
                return false;
            }
        };

        let sell_native = u256_to_big_decimal(&sell_native);
        let buy_native = u256_to_big_decimal(&buy_native);
        if sell_native >= buy_native * price_factor {
            true
        } else {
            tracing::debug!(
                order_uid = %order.metadata.uid,
                "limit order is outside market price, skipping",
            );
            false
        }
    });
}

/// A helper struct to apply filters for orders and report metrics.
struct OrderFilter {
    metrics: &'static Metrics,
    orders: Vec<Order>,
    counts: HashMap<OrderClassName, ClassCount>,
}

type OrderClassName = &'static str;

#[derive(Default)]
struct ClassCount {
    count: usize,
    filtered: HashMap<&'static str, usize>,
}

impl OrderFilter {
    /// Creates a new order filter.
    fn new(metrics: &'static Metrics, orders: Vec<Order>) -> Self {
        let counts = orders.iter().fold(
            OrderClass::VARIANTS
                .iter()
                .map(|class| (*class, ClassCount::default()))
                .collect::<HashMap<_, _>>(),
            |mut counts, order| {
                counts
                    .get_mut(order.metadata.class.as_ref())
                    .expect("count initialized for all order classes")
                    .count += 1;
                counts
            },
        );

        Self {
            metrics,
            orders,
            counts,
        }
    }

    /// Helper method for creating an order filter for tests.
    #[cfg(test)]
    fn test(orders: Vec<Order>) -> Self {
        let metrics = Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap();
        Self::new(metrics, orders)
    }

    /// Returns a slice into the current orders in the filter.
    fn as_slice(&self) -> &[Order] {
        &self.orders
    }

    /// Returns a mutable slice into the current orders in the filter.
    fn as_slice_mut(&mut self) -> &mut [Order] {
        &mut self.orders
    }

    /// Retain orders based on a predicate.
    fn retain(&mut self, reason: &'static str, predicate: impl FnMut(&Order) -> bool) {
        self.filter(reason, |mut orders| {
            orders.retain(predicate);
            orders
        });
    }

    /// Retain orders based on an async predicate.
    async fn try_retain_async<F, Fut>(
        &mut self,
        reason: &'static str,
        mut predicate: F,
    ) -> Result<()>
    where
        F: FnMut(&Order) -> Fut,
        Fut: Future<Output = Result<bool>>,
    {
        self.try_filter_async(reason, |mut orders| async move {
            let mut index = 0;
            'outer: while index < orders.len() {
                if !predicate(&orders[index]).await? {
                    orders.swap_remove(index);
                    continue 'outer;
                }
                index += 1;
            }
            Ok(orders)
        })
        .await
    }

    /// Filter orders and store metrics.
    fn filter<F>(&mut self, reason: &'static str, apply: F)
    where
        F: FnOnce(Vec<Order>) -> Vec<Order>,
    {
        self.try_filter(reason, |orders| Ok(apply(orders))).unwrap();
    }

    /// Filter orders and store metrics propagating errors.
    fn try_filter<F>(&mut self, reason: &'static str, apply: F) -> Result<()>
    where
        F: FnOnce(Vec<Order>) -> Result<Vec<Order>>,
    {
        self.try_filter_async(reason, |orders| async move { apply(orders) })
            .now_or_never()
            .expect("synchronous order filter did not resolve immediately")
    }

    /// Asyncronously filters orders and store metrics.
    async fn try_filter_async<F, Fut>(&mut self, reason: &'static str, apply: F) -> Result<()>
    where
        F: FnOnce(Vec<Order>) -> Fut,
        Fut: Future<Output = Result<Vec<Order>>>,
    {
        self.orders = apply(mem::take(&mut self.orders)).await?;

        let new_counts = self
            .orders
            .iter()
            .counts_by(|order| order.metadata.class.as_ref());
        for (class, new_count) in new_counts {
            let group = self
                .counts
                .get_mut(class)
                .expect("count initialized for all order classes");
            let filtered = group.count - new_count;

            group.count = new_count;
            *group.filtered.entry(reason).or_default() += filtered;
        }

        Ok(())
    }

    /// Finishes order filtering and reports metrics.
    fn finish(self) -> Vec<Order> {
        self.metrics.auction_creations.inc();
        for class in OrderClass::VARIANTS {
            let correct_class = |order: &&Order| &order.metadata.class.as_ref() == class;
            let solvable_grouped_by_class = self.orders.iter().filter(correct_class).count();
            self.metrics
                .auction_solvable_orders
                .with_label_values(&[class])
                .set(solvable_grouped_by_class as i64);
            let filtered_grouped_by_class = self.orders.iter().filter(correct_class).count();
            self.metrics
                .auction_filtered_orders
                .with_label_values(&[class])
                .set(filtered_grouped_by_class as i64);
        }

        self.orders
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDateTime, Utc};
    use futures::{FutureExt, StreamExt};
    use maplit::{btreemap, hashmap, hashset};
    use mockall::predicate::eq;
    use model::order::{
        LimitOrderClass, OrderBuilder, OrderData, OrderKind, OrderMetadata, OrderUid,
    };
    use primitive_types::H160;
    use shared::{
        bad_token::list_based::ListBasedDetector,
        price_estimation::{native::MockNativePriceEstimating, PriceEstimationError},
        signature_validator::{MockSignatureValidating, SignatureValidationError},
    };

    #[tokio::test]
    async fn filters_insufficient_balances() {
        let mut orders = vec![
            Order {
                data: OrderData {
                    sell_amount: 3.into(),
                    fee_amount: 3.into(),
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(2, 0), Utc),
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                data: OrderData {
                    sell_amount: 2.into(),
                    fee_amount: 2.into(),
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let balances = hashmap! {Query::from_order(&orders[0]) => U256::from(9)};

        let mut order_filter = OrderFilter::test(orders.clone());
        solvable_orders(&mut order_filter, &balances, None);
        // Second order has lower timestamp so it isn't picked.
        assert_eq!(order_filter.finish(), orders[..1]);

        orders[1].metadata.creation_date =
            DateTime::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc);

        let mut order_filter = OrderFilter::test(orders.clone());
        solvable_orders(&mut order_filter, &balances, None);
        assert_eq!(order_filter.finish(), orders[1..]);
    }

    #[tokio::test]
    async fn do_not_filters_insufficient_balances_for_ethflow_orders() {
        let ethflow_address = H160([3u8; 20]);
        let orders = vec![Order {
            data: OrderData {
                sell_amount: 3.into(),
                fee_amount: 3.into(),
                ..Default::default()
            },
            metadata: OrderMetadata {
                creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(2, 0), Utc),
                owner: ethflow_address,
                ..Default::default()
            },
            ..Default::default()
        }];

        let balances = hashmap! {Query::from_order(&orders[0]) => U256::from(0)};

        let mut order_filter = OrderFilter::test(orders.clone());
        solvable_orders(&mut order_filter, &balances, Some(ethflow_address));
        assert_eq!(order_filter.finish(), orders);
    }

    #[test]
    fn computes_u256_prices_normalized_to_1e18() {
        assert_eq!(
            to_normalized_price(0.5).unwrap(),
            U256::from(500_000_000_000_000_000_u128),
        );
    }

    #[test]
    fn normalize_prices_fail_when_outside_valid_input_range() {
        assert!(to_normalized_price(0.).is_none());
        assert!(to_normalized_price(-1.).is_none());
        assert!(to_normalized_price(f64::INFINITY).is_none());

        let min_price = 1. / 1e18;
        assert!(to_normalized_price(min_price).is_some());
        assert!(to_normalized_price(min_price * (1. - f64::EPSILON)).is_none());

        let uint_max = 2.0_f64.powi(256);
        let max_price = uint_max / 1e18;
        assert!(to_normalized_price(max_price).is_none());
        assert!(to_normalized_price(max_price * (1. - f64::EPSILON)).is_some());
    }

    #[tokio::test]
    async fn filters_tokens_without_native_prices() {
        let token1 = H160([1; 20]);
        let token2 = H160([2; 20]);
        let token3 = H160([3; 20]);
        let token4 = H160([4; 20]);

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
        let prices = btreemap! {
            token1 => 2.,
            token3 => 0.25,
            token4 => 0., // invalid price!
        };

        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            // deal with undeterministic ordering of `HashSet`.
            .withf(move |tokens| {
                tokens.iter().cloned().collect::<HashSet<_>>()
                    == hashset!(token1, token2, token3, token4)
            })
            .returning({
                let prices = prices.clone();
                move |tokens| {
                    let results = tokens
                        .iter()
                        .map(|token| {
                            prices
                                .get(token)
                                .copied()
                                .ok_or(PriceEstimationError::NoLiquidity)
                        })
                        .enumerate()
                        .collect::<Vec<_>>();
                    futures::stream::iter(results).boxed()
                }
            });

        let mut order_filter = OrderFilter::test(orders.clone());
        let prices = filter_orders_with_native_prices(
            &mut order_filter,
            &native_price_estimator,
            Instant::now() + MAX_AUCTION_CREATION_TIME,
            Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
        )
        .await;

        assert_eq!(order_filter.finish(), [orders[2].clone()]);
        assert_eq!(
            prices,
            btreemap! {
                token1 => U256::from(2_000_000_000_000_000_000_u128),
                token3 => U256::from(250_000_000_000_000_000_u128),
            }
        );
    }

    #[test]
    fn computes_max_transfer_out_amount_for_order() {
        // For fill-or-kill orders, we don't overflow even for very large buy
        // orders (where `{sell,fee}_amount * buy_amount` would overflow).
        assert_eq!(
            max_transfer_out_amount(&Order {
                data: OrderData {
                    sell_amount: 1000.into(),
                    fee_amount: 337.into(),
                    buy_amount: U256::MAX,
                    kind: OrderKind::Buy,
                    partially_fillable: false,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap(),
            U256::from(1337),
        );

        // Partially filled order scales amount.
        assert_eq!(
            max_transfer_out_amount(&Order {
                data: OrderData {
                    sell_amount: 100.into(),
                    buy_amount: 10.into(),
                    fee_amount: 101.into(),
                    kind: OrderKind::Buy,
                    partially_fillable: true,
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    executed_buy_amount: 9_u32.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap(),
            U256::from(20),
        );
    }

    #[test]
    fn max_transfer_out_amount_overflow() {
        // For fill-or-kill orders, overflow if the total sell and fee amount
        // overflows a uint. This kind of order cannot be filled by the
        // settlement contract anyway.
        assert!(max_transfer_out_amount(&Order {
            data: OrderData {
                sell_amount: U256::MAX,
                fee_amount: 1.into(),
                partially_fillable: false,
                ..Default::default()
            },
            ..Default::default()
        })
        .is_err());

        // Handles overflow when computing fill ratio.
        assert!(max_transfer_out_amount(&Order {
            data: OrderData {
                sell_amount: 1000.into(),
                fee_amount: 337.into(),
                buy_amount: U256::MAX,
                kind: OrderKind::Buy,
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        })
        .is_err());
    }

    #[tokio::test(start_paused = true)]
    async fn native_prices_uses_timeout() {
        shared::tracing::initialize_for_tests("debug");
        let mut native_price_estimator = MockNativePriceEstimating::new();
        native_price_estimator
            .expect_estimate_native_prices()
            .returning(move |tokens| {
                #[allow(clippy::unnecessary_to_owned)]
                let results = tokens
                    .to_vec()
                    .into_iter()
                    .enumerate()
                    .map(|(i, _)| (i, Ok(1.0)));
                futures::stream::iter(results)
                    .then(|price| async {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        price
                    })
                    .boxed()
            });
        let orders = vec![
            OrderBuilder::default()
                .with_sell_token(H160::from_low_u64_be(0))
                .with_buy_token(H160::from_low_u64_be(1))
                .build(),
            OrderBuilder::default()
                .with_sell_token(H160::from_low_u64_be(2))
                .with_buy_token(H160::from_low_u64_be(3))
                .build(),
        ];
        // last token price won't be available
        let deadline = Instant::now() + Duration::from_secs_f32(3.5);
        let mut order_filter = OrderFilter::test(orders.clone());
        let prices = filter_orders_with_native_prices(
            &mut order_filter,
            &native_price_estimator,
            deadline,
            Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
        )
        .await;

        let orders_ = order_filter.finish();
        assert_eq!(orders_.len(), 1);
        // It is not guaranteed which order is the included one because the function uses a hashset
        // for the tokens.
        assert!(orders_[0] == orders[0] || orders_[0] == orders[1]);
        assert_eq!(prices.len(), 2);
        assert!(prices.contains_key(&orders_[0].data.sell_token));
        assert!(prices.contains_key(&orders_[0].data.buy_token));
    }

    #[test]
    fn filters_banned_users() {
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
        .map(|owner| Order {
            metadata: OrderMetadata {
                owner,
                ..Default::default()
            },
            data: OrderData {
                buy_amount: 1.into(),
                sell_amount: 1.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .collect();

        let mut order_filter = OrderFilter::test(orders);
        filter_banned_user_orders(&mut order_filter, &banned_users);

        let filtered_owners = order_filter
            .as_slice()
            .iter()
            .map(|order| order.metadata.owner)
            .collect::<Vec<_>>();
        assert_eq!(
            filtered_owners,
            [H160([1; 20]), H160([1; 20]), H160([2; 20]), H160([3; 20])],
        );
    }

    #[test]
    fn filters_zero_amount_orders() {
        let orders = vec![
            // normal order with non zero amounts
            Order {
                data: OrderData {
                    buy_amount: 1u8.into(),
                    sell_amount: 1u8.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            // partially fillable order with remaining liquidity
            Order {
                data: OrderData {
                    partially_fillable: true,
                    buy_amount: 1u8.into(),
                    sell_amount: 1u8.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
            // normal order with zero amounts
            Order::default(),
            // partially fillable order completely filled
            Order {
                metadata: OrderMetadata {
                    executed_buy_amount: 1u8.into(),
                    executed_sell_amount: 1u8.into(),
                    ..Default::default()
                },
                data: OrderData {
                    partially_fillable: true,
                    buy_amount: 1u8.into(),
                    sell_amount: 1u8.into(),
                    ..Default::default()
                },
                ..Default::default()
            },
        ];

        let balances = hashmap! {Query::from_order(&orders[0]) => U256::MAX};
        let expected_result = vec![orders[0].clone(), orders[1].clone()];

        let mut order_filter = OrderFilter::test(orders);
        solvable_orders(&mut order_filter, &balances, None);

        let mut filtered_orders = order_filter.finish();
        // Deal with `solvable_orders()` sorting the orders.
        filtered_orders.sort_by_key(|order| order.metadata.creation_date);

        assert_eq!(expected_result, filtered_orders);
    }

    #[tokio::test]
    async fn filters_invalidated_eip1271_signatures() {
        let orders = vec![
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([1; 32]), H160([11; 20]), 1),
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                metadata: OrderMetadata {
                    uid: OrderUid::from_parts(H256([2; 32]), H160([22; 20]), 2),
                    ..Default::default()
                },
                signature: Signature::Eip1271(vec![2, 2]),
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
                },
                SignatureCheck {
                    signer: H160([44; 20]),
                    hash: [4; 32],
                    signature: vec![4, 4, 4, 4],
                },
                SignatureCheck {
                    signer: H160([55; 20]),
                    hash: [5; 32],
                    signature: vec![5, 5, 5, 5, 5],
                },
            ]))
            .returning(|_| vec![Ok(()), Err(SignatureValidationError::Invalid), Ok(())]);

        let mut orders = OrderFilter::test(orders);
        filter_invalid_signature_orders(&mut orders, &signature_validator).await;
        let remaining_uids = orders
            .as_slice()
            .iter()
            .map(|order| order.metadata.uid)
            .collect::<Vec<_>>();

        assert_eq!(
            remaining_uids,
            vec![
                OrderUid::from_parts(H256([1; 32]), H160([11; 20]), 1),
                OrderUid::from_parts(H256([2; 32]), H160([22; 20]), 2),
                OrderUid::from_parts(H256([3; 32]), H160([33; 20]), 3),
                OrderUid::from_parts(H256([5; 32]), H160([55; 20]), 5),
            ]
        );
    }

    #[test]
    fn filter_unsupported_tokens_() {
        let token0 = H160::from_low_u64_le(0);
        let token1 = H160::from_low_u64_le(1);
        let token2 = H160::from_low_u64_le(2);
        let bad_token = ListBasedDetector::deny_list(vec![token0]);
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

        let mut order_filter = OrderFilter::test(orders.clone());
        filter_unsupported_tokens(&mut order_filter, &bad_token)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(order_filter.as_slice(), &orders[1..2]);
    }

    #[test]
    fn filters_limit_orders_with_too_high_fees() {
        let order = |sell_amount: u8, surplus_fee: u8| Order {
            data: OrderData {
                buy_amount: 1u8.into(),
                sell_amount: sell_amount.into(),
                ..Default::default()
            },
            metadata: OrderMetadata {
                class: OrderClass::Limit(LimitOrderClass {
                    surplus_fee: surplus_fee.into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };
        let mut orders = OrderFilter::test(vec![
            // Enough sell amout for the surplus fee.
            order(100, 10),
            // Surplus fee effectively turns order into a 0 sell amount order
            order(100, 100),
            // Surplus fee is higher than the sell amount.
            order(100, 101),
        ]);
        filter_limit_orders_with_insufficient_sell_amount(&mut orders);

        assert_eq!(orders.as_slice(), [order(100, 10)]);
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

        let order = |sell_amount: u8, buy_amount: u8, surplus_fee: u8| Order {
            data: OrderData {
                sell_token,
                sell_amount: sell_amount.into(),
                buy_token,
                buy_amount: buy_amount.into(),
                ..Default::default()
            },
            metadata: OrderMetadata {
                class: OrderClass::Limit(LimitOrderClass {
                    surplus_fee: surplus_fee.into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
            ..Default::default()
        };

        let valid_orders = vec![
            // Reasonably priced order, doesn't get filtered.
            order(101, 200, 1),
            // Slightly out of price order, doesn't get filtered.
            order(10, 21, 0),
        ];

        let invalid_orders = vec![
            // Out of price order gets filtered out.
            order(10, 100, 0),
            // Reasonably priced order becomes out of price after fees and gets
            // filtered out
            order(10, 18, 5),
            // Zero sell amount after fees gets filtered.
            order(1, 1, 1),
            // Overflow sell amount after fees gets filtered.
            order(1, 1, 100),
            // Overflow sell value gets filtered.
            order(255, 1, 1),
            // Overflow buy value gets filtered.
            order(100, 255, 1),
        ];

        let mut orders = OrderFilter::test([valid_orders.clone(), invalid_orders].concat());
        filter_mispriced_limit_orders(&mut orders, &prices, &price_factor);

        assert_eq!(orders.as_slice(), valid_orders);
    }
}
