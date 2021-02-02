use std::time::SystemTime;

use anyhow::Result;

use crate::storage::{AddOrderResult, RemoveOrderResult, Storage};
use crate::{account_balances::BalanceFetching, database::OrderFilter};
use contracts::GPv2Settlement;
use futures::join;
use model::{
    order::{Order, OrderCreation, OrderUid},
    DomainSeparator,
};

pub struct Orderbook {
    domain_separator: DomainSeparator,
    storage: Box<dyn Storage>,
    balance_fetcher: Box<dyn BalanceFetching>,
}

impl Orderbook {
    pub fn new(
        domain_separator: DomainSeparator,
        storage: Box<dyn Storage>,
        balance_fetcher: Box<dyn BalanceFetching>,
    ) -> Self {
        Self {
            domain_separator,
            storage,
            balance_fetcher,
        }
    }

    pub async fn add_order(&self, order: OrderCreation) -> Result<AddOrderResult> {
        if !has_future_valid_to(now_in_epoch_seconds(), &order) {
            return Ok(AddOrderResult::PastValidTo);
        }
        let order = match Order::from_order_creation(order, &self.domain_separator) {
            Some(order) => order,
            None => return Ok(AddOrderResult::InvalidSignature),
        };
        self.balance_fetcher
            .register(order.order_meta_data.owner, order.order_creation.sell_token)
            .await;
        self.storage.add_order(order).await
    }

    pub async fn remove_order(&self, uid: &OrderUid) -> Result<RemoveOrderResult> {
        self.storage.remove_order(uid).await
    }

    pub async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        let mut orders_without_balance = self.storage.get_orders(filter).await?;

        // Since order can come from storage after a cold start there is the possibility that they are not yet registered
        // for balance updates. In this case we do it here.
        let untracked = orders_without_balance
            .iter()
            .filter_map(|order| {
                match self
                    .balance_fetcher
                    .get_balance(order.order_meta_data.owner, order.order_creation.sell_token)
                {
                    Some(_) => None,
                    None => Some((order.order_meta_data.owner, order.order_creation.sell_token)),
                }
            })
            .collect();
        self.balance_fetcher.register_many(untracked).await;

        // Enrich orders with balance information
        for order in orders_without_balance.iter_mut() {
            order.order_meta_data.available_balance = self
                .balance_fetcher
                .get_balance(order.order_meta_data.owner, order.order_creation.sell_token);
        }
        Ok(orders_without_balance)
    }

    pub async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) -> Result<()> {
        join!(
            self.storage.run_maintenance(settlement_contract),
            self.balance_fetcher.update()
        )
        .0
    }
}

pub fn now_in_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("now earlier than epoch")
        .as_secs()
}

pub fn has_future_valid_to(now_in_epoch_seconds: u64, order: &OrderCreation) -> bool {
    order.valid_to as u64 > now_in_epoch_seconds
}

#[cfg(test)]
mod tests {
    use ethcontract::H160;
    use mockall::{
        predicate::{always, eq},
        Sequence,
    };
    use model::{order::OrderCreation, DomainSeparator};

    use super::*;
    use crate::account_balances::MockBalanceFetching;
    use crate::storage::{InMemoryOrderBook, MockStorage};

    #[tokio::test]
    async fn watches_owners_sell_token_balance_for_added_orders() {
        let storage = Box::new(InMemoryOrderBook::default());
        let mut balance_fetcher = MockBalanceFetching::new();

        let sell_token = H160::from_low_u64_be(2);
        let order = OrderCreation {
            sell_token,
            ..Default::default()
        };

        balance_fetcher
            .expect_register()
            .with(always(), eq(sell_token))
            .returning(|_, _| ());

        let orderbook = Orderbook::new(
            DomainSeparator::default(),
            storage,
            Box::new(balance_fetcher),
        );
        orderbook.add_order(order).await.unwrap();
    }

    #[tokio::test]
    async fn enriches_storage_orders_with_available_balance() {
        let mut storage = MockStorage::new();
        let mut balance_fetcher = MockBalanceFetching::new();

        let sell_token = H160::from_low_u64_be(2);
        let balance = 100.into();

        let orders = vec![Order {
            order_creation: OrderCreation {
                sell_token,
                ..Default::default()
            },
            ..Default::default()
        }];

        let storage_orders = orders.clone();
        storage
            .expect_get_orders()
            .return_once(|_| Ok(storage_orders));

        balance_fetcher
            .expect_register_many()
            .with(eq(vec![]))
            .returning(|_| ());
        balance_fetcher
            .expect_get_balance()
            .with(eq(orders[0].order_meta_data.owner), eq(sell_token))
            .return_const(Some(balance));

        let orderbook = Orderbook::new(
            DomainSeparator::default(),
            Box::new(storage),
            Box::new(balance_fetcher),
        );
        let orders = orderbook.get_orders(&OrderFilter::default()).await.unwrap();
        assert_eq!(orders[0].order_meta_data.available_balance, Some(balance));
    }

    #[tokio::test]
    async fn resgisters_untracked_balances_on_fetching() {
        let mut storage = MockStorage::new();
        let mut balance_fetcher = MockBalanceFetching::new();

        let a_sell_token = H160::from_low_u64_be(2);
        let a_balance = 100.into();

        let another_sell_token = H160::from_low_u64_be(3);
        let another_balance = 200.into();

        let orders = vec![
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

        let storage_orders = orders.clone();
        storage
            .expect_get_orders()
            .return_once(|_| Ok(storage_orders));

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

        let orderbook = Orderbook::new(
            DomainSeparator::default(),
            Box::new(storage),
            Box::new(balance_fetcher),
        );
        let orders = orderbook.get_orders(&OrderFilter::default()).await.unwrap();
        assert_eq!(orders[0].order_meta_data.available_balance, Some(a_balance));
        assert_eq!(
            orders[1].order_meta_data.available_balance,
            Some(another_balance)
        );
    }
}
