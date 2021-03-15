use crate::{
    account_balances::BalanceFetching, database::OrderFilter, event_updater::EventUpdater,
};
use crate::{
    database::{Database, InsertionError},
    fee::MinFeeCalculator,
};
use anyhow::Result;
use contracts::GPv2Settlement;
use futures::{join, TryStreamExt};
use model::order::OrderCancellation;
use model::{
    order::{Order, OrderCreation, OrderUid},
    DomainSeparator, EIP712Signing,
};
use shared::time::now_in_epoch_seconds;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Eq, PartialEq)]
pub enum AddOrderResult {
    Added(OrderUid),
    DuplicatedOrder,
    InvalidSignature,
    Forbidden,
    MissingOrderData,
    PastValidTo,
    InsufficientFunds,
    InsufficientFee,
}

#[derive(Debug)]
pub enum OrderCancellationResult {
    Cancelled,
    InvalidSignature,
    WrongOwner,
    OrderNotFound,
}

pub struct Orderbook {
    domain_separator: DomainSeparator,
    database: Database,
    event_updater: Mutex<EventUpdater>,
    balance_fetcher: Box<dyn BalanceFetching>,
    fee_validator: Arc<MinFeeCalculator>,
}

impl Orderbook {
    pub fn new(
        domain_separator: DomainSeparator,
        database: Database,
        event_updater: EventUpdater,
        balance_fetcher: Box<dyn BalanceFetching>,
        fee_validator: Arc<MinFeeCalculator>,
    ) -> Self {
        Self {
            domain_separator,
            database,
            event_updater: Mutex::new(event_updater),
            balance_fetcher,
            fee_validator,
        }
    }

    pub async fn add_order(&self, order: OrderCreation) -> Result<AddOrderResult> {
        if !has_future_valid_to(shared::time::now_in_epoch_seconds(), &order) {
            return Ok(AddOrderResult::PastValidTo);
        }
        if !self
            .fee_validator
            .is_valid_fee(order.sell_token, order.fee_amount)
            .await
        {
            return Ok(AddOrderResult::InsufficientFee);
        }
        let order = match Order::from_order_creation(order, &self.domain_separator) {
            Some(order) => order,
            None => return Ok(AddOrderResult::InvalidSignature),
        };
        self.balance_fetcher
            .register(order.order_meta_data.owner, order.order_creation.sell_token)
            .await;
        match self.database.insert_order(&order).await {
            Ok(()) => Ok(AddOrderResult::Added(order.order_meta_data.uid)),
            Err(InsertionError::DuplicatedRecord) => Ok(AddOrderResult::DuplicatedOrder),
            Err(InsertionError::DbError(err)) => Err(err.into()),
        }
    }

    pub async fn cancel_order(
        &self,
        cancellation: OrderCancellation,
    ) -> Result<OrderCancellationResult> {
        // TODO - Would like to use get_order_by_uid, but not implemented on self
        let orders = self
            .get_orders(&OrderFilter {
                uid: Some(cancellation.order_uid),
                ..Default::default()
            })
            .await?;
        // Could be that order doesn't exist and is not fetched.
        let order = match orders.first() {
            Some(order) => order,
            None => return Ok(OrderCancellationResult::OrderNotFound),
        };

        match cancellation.validate_signature(&self.domain_separator) {
            Some(signer) => {
                if signer == order.order_meta_data.owner {
                    // order is already known to exist in DB at this point!
                    self.database
                        .cancel_order(&order.order_meta_data.uid)
                        .await?;
                    Ok(OrderCancellationResult::Cancelled)
                } else {
                    Ok(OrderCancellationResult::WrongOwner)
                }
            }
            None => Ok(OrderCancellationResult::InvalidSignature),
        }
    }

    pub async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        let mut orders = self.database.orders(filter).try_collect::<Vec<_>>().await?;
        set_order_balance(orders.as_mut_slice(), self.balance_fetcher.as_ref()).await;
        if filter.exclude_insufficient_balance {
            remove_orders_without_sufficient_balance(&mut orders);
        }
        Ok(orders)
    }

    pub async fn get_solvable_orders(&self) -> Result<Vec<Order>> {
        let filter = OrderFilter {
            min_valid_to: now_in_epoch_seconds(),
            exclude_fully_executed: true,
            exclude_invalidated: true,
            exclude_insufficient_balance: true,
            ..Default::default()
        };
        self.get_orders(&filter).await
    }

    pub async fn run_maintenance(&self, _settlement_contract: &GPv2Settlement) -> Result<()> {
        let update_events = async { self.event_updater.lock().await.update_events().await };
        join!(update_events, self.balance_fetcher.update()).0
    }
}

fn has_future_valid_to(now_in_epoch_seconds: u32, order: &OrderCreation) -> bool {
    order.valid_to > now_in_epoch_seconds
}

async fn set_order_balance(orders: &mut [Order], balance_fetcher: &dyn BalanceFetching) {
    // Since order can come from storage after a cold start there is the possibility that they are not yet registered
    // for balance updates. In this case we do it here.
    let untracked = orders
        .iter()
        .filter_map(|order| {
            match balance_fetcher
                .get_balance(order.order_meta_data.owner, order.order_creation.sell_token)
            {
                Some(_) => None,
                None => Some((order.order_meta_data.owner, order.order_creation.sell_token)),
            }
        })
        .collect();
    balance_fetcher.register_many(untracked).await;

    // Enrich orders with balance information
    for order in orders.iter_mut() {
        order.order_meta_data.available_balance = balance_fetcher
            .get_balance(order.order_meta_data.owner, order.order_creation.sell_token);
    }
}

fn remove_orders_without_sufficient_balance(orders: &mut Vec<Order>) {
    orders.retain(|order| {
        let balance = order.order_meta_data.available_balance.unwrap_or_default();
        !balance.is_zero()
            && (order.order_creation.partially_fillable
                || balance >= (order.order_creation.sell_amount + order.order_creation.fee_amount))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_balances::MockBalanceFetching;
    use ethcontract::H160;
    use mockall::{predicate::eq, Sequence};
    use model::order::{OrderCreation, OrderMetaData};

    #[tokio::test]
    async fn enriches_storage_orders_with_available_balance() {
        let mut balance_fetcher = MockBalanceFetching::new();

        let sell_token = H160::from_low_u64_be(2);
        let balance = 100.into();

        let mut orders = vec![Order {
            order_creation: OrderCreation {
                sell_token,
                ..Default::default()
            },
            ..Default::default()
        }];

        balance_fetcher
            .expect_register_many()
            .with(eq(vec![]))
            .returning(|_| ());
        balance_fetcher
            .expect_get_balance()
            .with(eq(orders[0].order_meta_data.owner), eq(sell_token))
            .return_const(Some(balance));

        set_order_balance(orders.as_mut_slice(), &balance_fetcher).await;
        assert_eq!(orders[0].order_meta_data.available_balance, Some(balance));
    }

    #[tokio::test]
    async fn registers_untracked_balances_on_fetching() {
        let mut balance_fetcher = MockBalanceFetching::new();

        let a_sell_token = H160::from_low_u64_be(2);
        let a_balance = 100.into();

        let another_sell_token = H160::from_low_u64_be(3);
        let another_balance = 200.into();

        let mut orders = vec![
            Order {
                order_creation: OrderCreation {
                    sell_token: a_sell_token,
                    ..Default::default()
                },
                ..Default::default()
            },
            Order {
                order_creation: OrderCreation {
                    sell_token: another_sell_token,
                    ..Default::default()
                },
                ..Default::default()
            },
        ];
        let owner = orders[0].order_meta_data.owner;

        balance_fetcher
            .expect_get_balance()
            .with(eq(owner), eq(a_sell_token))
            .return_const(Some(a_balance));

        // Not having a balance for the second order, should trigger a register_many only for this token
        let mut seq = Sequence::new();
        balance_fetcher
            .expect_get_balance()
            .with(eq(owner), eq(another_sell_token))
            .times(1)
            .in_sequence(&mut seq)
            .return_const(None);

        balance_fetcher
            .expect_register_many()
            .with(eq(vec![(owner, another_sell_token)]))
            .times(1)
            .in_sequence(&mut seq)
            .returning(|_| ());

        // Once registered, we can return the balance
        balance_fetcher
            .expect_get_balance()
            .with(eq(owner), eq(another_sell_token))
            .times(1)
            .in_sequence(&mut seq)
            .return_const(Some(another_balance));

        set_order_balance(orders.as_mut_slice(), &balance_fetcher).await;
        assert_eq!(orders[0].order_meta_data.available_balance, Some(a_balance));
        assert_eq!(
            orders[1].order_meta_data.available_balance,
            Some(another_balance)
        );
    }

    #[tokio::test]
    async fn filters_insufficient_balances() {
        let mut orders = vec![
            Order {
                order_creation: OrderCreation {
                    sell_amount: 100.into(),
                    partially_fillable: true,
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    available_balance: Some(50.into()),
                    ..Default::default()
                },
            },
            Order {
                order_creation: OrderCreation {
                    sell_amount: 200.into(),
                    partially_fillable: false,
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    available_balance: Some(50.into()),
                    ..Default::default()
                },
            },
            // Fee + sell amount > balance
            Order {
                order_creation: OrderCreation {
                    sell_amount: 200.into(),
                    fee_amount: 20.into(),
                    partially_fillable: false,
                    ..Default::default()
                },
                order_meta_data: OrderMetaData {
                    available_balance: Some(210.into()),
                    ..Default::default()
                },
            },
        ];

        remove_orders_without_sufficient_balance(&mut orders);

        // Only the partially fillable order is included
        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].order_creation.partially_fillable, true);
    }
}
