use crate::{
    account_balances::{BalanceFetching, Query},
    database::orders::OrderStoring,
    orderbook::filter_unsupported_tokens,
};
use anyhow::Result;
use model::order::Order;
use primitive_types::U256;
use shared::{
    bad_token::BadTokenDetecting, current_block::CurrentBlockStream, time::now_in_epoch_seconds,
};
use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use tokio::sync::Notify;

/// Keeps track and updates the set of currently solvable orders.
/// For this we also need to keep track of user sell token balances for open orders so this is
/// retrievable as well.
/// The cache is updated in the background whenever a new block appears or when the cache is
/// explicitly notified that it should update for example because a new order got added to the order
/// book.
pub struct SolvableOrdersCache {
    min_order_validity_period: Duration,
    database: Arc<dyn OrderStoring>,
    balance_fetcher: Arc<dyn BalanceFetching>,
    bad_token_detector: Arc<dyn BadTokenDetecting>,
    notify: Notify,
    cache: Mutex<Inner>,
}

type Balances = HashMap<Query, U256>;

struct Inner {
    orders: SolvableOrders,
    balances: Balances,
    block: u64,
}

#[derive(Clone, Debug)]
pub struct SolvableOrders {
    pub orders: Vec<Order>,
    pub update_time: Instant,
    pub latest_settlement_block: u64,
}

impl SolvableOrdersCache {
    pub fn new(
        min_order_validity_period: Duration,
        database: Arc<dyn OrderStoring>,
        balance_fetcher: Arc<dyn BalanceFetching>,
        bad_token_detector: Arc<dyn BadTokenDetecting>,
        current_block: CurrentBlockStream,
    ) -> Arc<Self> {
        let self_ = Arc::new(Self {
            min_order_validity_period,
            database,
            balance_fetcher,
            bad_token_detector,
            notify: Default::default(),
            cache: Mutex::new(Inner {
                orders: SolvableOrders {
                    orders: Vec::new(),
                    update_time: Instant::now(),
                    latest_settlement_block: 0,
                },
                balances: Default::default(),
                block: 0,
            }),
        });
        tokio::task::spawn(update_task(Arc::downgrade(&self_), current_block));
        self_
    }

    pub fn cached_balance(&self, key: &Query) -> Option<U256> {
        let inner = self.cache.lock().unwrap();
        inner.balances.get(key).copied()
    }

    /// Orders and timestamp at which last update happened.
    pub fn cached_solvable_orders(&self) -> SolvableOrders {
        self.cache.lock().unwrap().orders.clone()
    }

    /// The cache will update the solvable orders and missing balances as soon as possible.
    pub fn request_update(&self) {
        self.notify.notify_one();
    }

    /// Manually update solvable orders. Usually called by the background updating task.
    pub async fn update(&self, block: u64) -> Result<()> {
        let min_valid_to = now_in_epoch_seconds() + self.min_order_validity_period.as_secs() as u32;
        let db_solvable_orders = self.database.solvable_orders(min_valid_to).await?;
        let orders =
            filter_unsupported_tokens(db_solvable_orders.orders, self.bad_token_detector.as_ref())
                .await?;

        // If we update due to an explicit notification we can reuse existing balances as they
        // cannot have changed.
        let old_balances = {
            let inner = self.cache.lock().unwrap();
            if inner.block == block {
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
            order.order_meta_data.available_balance = new_balances.get(&query).copied();
        }

        *self.cache.lock().unwrap() = Inner {
            orders: SolvableOrders {
                orders,
                update_time: Instant::now(),
                latest_settlement_block: db_solvable_orders.latest_settlement_block,
            },
            balances: new_balances,
            block,
        };

        Ok(())
    }
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
    orders.sort_by_key(|order| std::cmp::Reverse(order.order_meta_data.creation_date));
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
            let needed_balance = match order
                .order_creation
                .sell_amount
                .checked_add(order.order_creation.fee_amount)
            {
                Some(balance) => balance,
                None => continue,
            };
            if let Some(balance) = remaining_balance.checked_sub(needed_balance) {
                remaining_balance = balance;
                result.push(order);
            }
        }
    }
    result
}

/// Keep updating the cache every N seconds or when an update notification happens.
/// Exits when this becomes the only reference to the cache.
async fn update_task(cache: Weak<SolvableOrdersCache>, current_block: CurrentBlockStream) {
    loop {
        let cache = match cache.upgrade() {
            Some(self_) => self_,
            None => {
                tracing::debug!("exiting solvable orders update task");
                break;
            }
        };
        {
            // We are not updating on block changes because
            // - the state of orders could change even when the block does not like when an order
            //   gets cancelled off chain
            // - the event updater takes some time to run and if we go first we would not update the
            //   orders with the most recent events.
            const UPDATE_INTERVAL: Duration = Duration::from_secs(2);
            let timeout = tokio::time::sleep(UPDATE_INTERVAL);
            let notified = cache.notify.notified();
            futures::pin_mut!(timeout);
            futures::pin_mut!(notified);
            futures::future::select(timeout, notified).await;
        }
        let block = match current_block.borrow().number {
            Some(block) => block.as_u64(),
            None => {
                tracing::error!("no block number");
                continue;
            }
        };
        match cache.update(block).await {
            Ok(()) => tracing::debug!("updated solvable orders"),
            Err(err) => tracing::error!(?err, "failed to update solvable orders"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account_balances::MockBalanceFetching, database::orders::MockOrderStoring,
        database::orders::SolvableOrders as DbOrders,
    };
    use chrono::{DateTime, NaiveDateTime, Utc};
    use maplit::hashmap;
    use model::order::{OrderCreation, OrderMetaData, SellTokenSource};
    use primitive_types::H160;

    #[tokio::test]
    async fn filters_insufficient_balances() {
        let mut orders = vec![
            Order {
                order_creation: OrderCreation {
                    sell_amount: 3.into(),
                    fee_amount: 3.into(),
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(2, 0), Utc),
                    ..Default::default()
                },
            },
            Order {
                order_creation: OrderCreation {
                    sell_amount: 2.into(),
                    fee_amount: 2.into(),
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    creation_date: DateTime::from_utc(NaiveDateTime::from_timestamp(0, 0), Utc),
                    ..Default::default()
                },
            },
        ];

        let balances = hashmap! {Query::from_order(&orders[0]) => U256::from(9)};
        let orders_ = solvable_orders(orders.clone(), &balances);
        // Second order has lower timestamp so it isn't picked.
        assert_eq!(orders_, orders[..1]);
        orders[1].order_meta_data.creation_date =
            DateTime::from_utc(NaiveDateTime::from_timestamp(3, 0), Utc);
        let orders_ = solvable_orders(orders.clone(), &balances);
        assert_eq!(orders_, orders[1..]);
    }

    #[tokio::test]
    async fn caches_orders_and_balances() {
        let mut balance_fetcher = MockBalanceFetching::new();
        let mut order_storing = MockOrderStoring::new();
        let (_, receiver) = tokio::sync::watch::channel(Default::default());
        let bad_token_detector =
            shared::bad_token::list_based::ListBasedDetector::deny_list(Vec::new());

        let owner = H160::from_low_u64_le(0);
        let sell_token_0 = H160::from_low_u64_le(1);
        let sell_token_1 = H160::from_low_u64_le(2);

        let orders = [
            Order {
                order_creation: OrderCreation {
                    sell_token: sell_token_0,
                    sell_token_balance: SellTokenSource::Erc20,
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    owner,
                    ..Default::default()
                },
            },
            Order {
                order_creation: OrderCreation {
                    sell_token: sell_token_1,
                    sell_token_balance: SellTokenSource::Erc20,
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    owner,
                    ..Default::default()
                },
            },
        ];

        order_storing
            .expect_solvable_orders()
            .times(1)
            .return_once({
                let orders = orders.clone();
                move |_| {
                    Ok(DbOrders {
                        orders: vec![orders[0].clone()],
                        latest_settlement_block: 0,
                    })
                }
            });
        order_storing
            .expect_solvable_orders()
            .times(1)
            .return_once({
                let orders = orders.clone();
                move |_| {
                    Ok(DbOrders {
                        orders: orders.into(),
                        latest_settlement_block: 0,
                    })
                }
            });
        order_storing
            .expect_solvable_orders()
            .times(1)
            .return_once(|_| {
                Ok(DbOrders {
                    orders: Vec::new(),
                    latest_settlement_block: 0,
                })
            });

        balance_fetcher
            .expect_get_balances()
            .times(1)
            .return_once(|_| vec![Ok(1.into())]);
        balance_fetcher
            .expect_get_balances()
            .times(1)
            .return_once(|_| vec![Ok(2.into())]);
        balance_fetcher
            .expect_get_balances()
            .times(1)
            .return_once(|_| Vec::new());

        let cache = SolvableOrdersCache::new(
            Duration::from_secs(0),
            Arc::new(order_storing),
            Arc::new(balance_fetcher),
            Arc::new(bad_token_detector),
            receiver,
        );

        cache.update(0).await.unwrap();
        assert_eq!(
            cache.cached_balance(&Query::from_order(&orders[0])),
            Some(1.into())
        );
        assert_eq!(cache.cached_balance(&Query::from_order(&orders[1])), None);
        let orders_ = cache.cached_solvable_orders().orders;
        assert_eq!(orders_.len(), 1);
        assert_eq!(orders_[0].order_meta_data.available_balance, Some(1.into()));

        cache.update(0).await.unwrap();
        assert_eq!(
            cache.cached_balance(&Query::from_order(&orders[0])),
            Some(1.into())
        );
        assert_eq!(
            cache.cached_balance(&Query::from_order(&orders[1])),
            Some(2.into())
        );
        let orders_ = cache.cached_solvable_orders().orders;
        assert_eq!(orders_.len(), 2);

        cache.update(0).await.unwrap();
        assert_eq!(cache.cached_balance(&Query::from_order(&orders[0])), None,);
        assert_eq!(cache.cached_balance(&Query::from_order(&orders[1])), None,);
        let orders_ = cache.cached_solvable_orders().orders;
        assert_eq!(orders_.len(), 0);
    }
}
