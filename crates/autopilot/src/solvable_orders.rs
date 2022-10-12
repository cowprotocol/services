use crate::{database::Postgres, risk_adjusted_rewards};
use anyhow::{Context as _, Result};
use futures::StreamExt;
use model::{auction::Auction, order::Order, signature::Signature, time::now_in_epoch_seconds};
use primitive_types::{H160, H256, U256};
use prometheus::{IntCounter, IntGauge};
use shared::{
    account_balances::{BalanceFetching, Query},
    bad_token::BadTokenDetecting,
    current_block::CurrentBlockStream,
    price_estimation::native::NativePriceEstimating,
    signature_validator::{SignatureCheck, SignatureValidating},
};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    iter::FromIterator,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use tokio::time::Instant;

// When creating the auction after solvable orders change we need to fetch native prices for a
// potentially large amount of tokens. This is the maximum amount of time we allot for this
// operation.
const MAX_AUCTION_CREATION_TIME: Duration = Duration::from_secs(10);

#[derive(prometheus_metric_storage::MetricStorage)]
pub struct Metrics {
    /// auction creations
    auction_creations: IntCounter,

    /// auction solvable orders
    auction_solvable_orders: IntGauge,

    /// auction filtered orders
    auction_filtered_orders: IntGauge,

    /// auction errored price estimates
    auction_errored_price_estimates: IntCounter,

    /// auction price estimate timeouts
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
    /// then concurrent calls might overwrite eachother's results.
    pub async fn update(&self, block: u64) -> Result<()> {
        let min_valid_to = now_in_epoch_seconds() + self.min_order_validity_period.as_secs() as u32;
        let db_solvable_orders = self.database.solvable_orders(min_valid_to).await?;
        let orders = filter_banned_user_orders(db_solvable_orders.orders, &self.banned_users);
        let orders = filter_unsupported_tokens(orders, self.bad_token_detector.as_ref()).await?;
        let orders =
            filter_invalid_signature_orders(orders, self.signature_validator.as_ref()).await;

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
        let (mut new_balances, missing_queries) = new_balances(&old_balances, &orders);
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

        let mut orders = solvable_orders(orders, &new_balances);
        for order in &mut orders {
            let query = Query::from_order(order);
            order.metadata.available_balance = new_balances.get(&query).copied();
        }

        // create auction
        let (orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &*self.native_price_estimator,
            Instant::now() + MAX_AUCTION_CREATION_TIME,
            self.metrics,
        )
        .await;
        let rewards = if let Some(calculator) = &self.reward_calculator {
            let rewards = calculator
                .calculate_many(&orders)
                .await
                .context("rewards")?;
            orders
            .iter()
            .zip(rewards)
            .filter_map(|(order, reward)| match reward {
                Ok(reward) if reward > 0. => Some((order.metadata.uid, reward)),
                Ok(_) => None,
                Err(err) => {
                    tracing::warn!(?order.metadata.uid, ?err, "error calculating risk adjusted reward");
                    None
                }
            })
            .collect()
        } else {
            Default::default()
        };
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
fn filter_banned_user_orders(mut orders: Vec<Order>, banned_users: &HashSet<H160>) -> Vec<Order> {
    orders.retain(|order| !banned_users.contains(&order.metadata.owner));
    orders
}

/// Filters EIP-1271 orders whose signatures are no longer validating.
async fn filter_invalid_signature_orders(
    orders: Vec<Order>,
    signature_validator: &dyn SignatureValidating,
) -> Vec<Order> {
    let checks = orders
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
        return orders;
    }

    let mut validations = signature_validator
        .validate_signatures(checks)
        .await
        .into_iter();
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
}

/// Returns existing balances and Vec of queries that need to be peformed.
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
fn solvable_orders(mut orders: Vec<Order>, balances: &Balances) -> Vec<Order> {
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
        let block = match current_block.borrow().number {
            Some(block) => block.as_u64(),
            None => {
                tracing::error!("no block number");
                continue;
            }
        };
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

async fn get_orders_with_native_prices(
    mut orders: Vec<Order>,
    native_price_estimator: &dyn NativePriceEstimating,
    deadline: Instant,
    metrics: &Metrics,
) -> (Vec<Order>, BTreeMap<H160, U256>) {
    let traded_tokens = orders
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

    let original_order_count = orders.len() as u64;
    // Filter both orders and prices so that we only return orders that have prices and prices that
    // have orders.
    let mut used_prices = BTreeMap::new();
    orders.retain(|order| {
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

    let solvable_orders = orders.len() as u64;
    let filtered_orders = original_order_count - solvable_orders;
    metrics.auction_creations.inc();
    metrics.auction_solvable_orders.set(solvable_orders as i64);
    if timeout {
        metrics.auction_price_estimate_timeouts.inc();
    }
    metrics.auction_filtered_orders.set(filtered_orders as i64);
    metrics
        .auction_errored_price_estimates
        .inc_by(errored_estimates);

    (orders, used_prices)
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
    mut orders: Vec<Order>,
    bad_token: &dyn BadTokenDetecting,
) -> Result<Vec<Order>> {
    // Can't use normal `retain` or `filter` because the bad token detection is async. So either
    // this manual iteration or conversion to stream.
    let mut index = 0;
    'outer: while index < orders.len() {
        for token in orders[index].data.token_pair().unwrap() {
            if !bad_token.detect(token).await?.is_good() {
                orders.swap_remove(index);
                continue 'outer;
            }
        }
        index += 1;
    }
    Ok(orders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{DateTime, NaiveDateTime, Utc};
    use futures::{FutureExt, StreamExt};
    use maplit::{btreemap, hashmap, hashset};
    use mockall::predicate::eq;
    use model::order::{OrderBuilder, OrderData, OrderKind, OrderMetadata, OrderUid};
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
        let orders_ = solvable_orders(orders.clone(), &balances);
        // Second order has lower timestamp so it isn't picked.
        assert_eq!(orders_, orders[..1]);
        orders[1].metadata.creation_date =
            DateTime::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc);
        let orders_ = solvable_orders(orders.clone(), &balances);
        assert_eq!(orders_, orders[1..]);
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

        let (filtered_orders, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            Instant::now() + MAX_AUCTION_CREATION_TIME,
            Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
        )
        .await;

        assert_eq!(filtered_orders, [orders[2].clone()]);
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
        let (orders_, prices) = get_orders_with_native_prices(
            orders.clone(),
            &native_price_estimator,
            deadline,
            Metrics::instance(global_metrics::get_metric_storage_registry()).unwrap(),
        )
        .await;
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

        let filtered_orders = filter_banned_user_orders(orders, &banned_users);
        let filtered_owners = filtered_orders
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
        let mut filtered_orders = solvable_orders(orders, &balances);
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

        let filtered = filter_invalid_signature_orders(orders, &signature_validator).await;
        let remaining_uids = filtered
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
        let result = filter_unsupported_tokens(orders.clone(), &bad_token)
            .now_or_never()
            .unwrap()
            .unwrap();
        assert_eq!(result, &orders[1..2]);
    }
}
