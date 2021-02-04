use super::*;
use crate::orderbook::has_future_valid_to;
use anyhow::Result;
use contracts::GPv2Settlement;
use futures::future;
use model::order::{Order, OrderUid};
use primitive_types::U256;
use shared::time::now_in_epoch_seconds;
use std::collections::{hash_map::Entry, HashMap};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Default)]
pub struct OrderBook {
    // TODO: Store more efficiently (for example HashMap) depending on functionality we need.
    orders: RwLock<HashMap<OrderUid, Order>>,
}

impl OrderBook {
    async fn remove_expired_orders(&self, now_in_epoch_seconds: u32) {
        // TODO: use the timestamp from the most recent block instead?
        let mut orders = self.orders.write().await;
        orders.retain(|_, order| has_future_valid_to(now_in_epoch_seconds, &order.order_creation));
    }

    async fn remove_settled_orders(&self, settlement_contract: &GPv2Settlement) {
        let uids_to_be_removed = self.get_uids_of_settled_orders(settlement_contract).await;
        info!(
            "removing following settled orders from orderbook: {:?}",
            uids_to_be_removed
        );
        let mut orders = self.orders.write().await;
        orders.retain(|uid, _| !uids_to_be_removed.contains(uid));
    }

    async fn get_uids_of_settled_orders(
        &self,
        settlement_contract: &GPv2Settlement,
    ) -> Vec<OrderUid> {
        let orders = self.orders.read().await;
        let uid_futures = orders.iter().map(|(uid, _)| async move {
            self.has_uid_been_settled(*uid, settlement_contract).await
        });
        let uid_pairs: Vec<Option<OrderUid>> = future::join_all(uid_futures).await;
        uid_pairs.iter().filter_map(|uid| *uid).collect()
    }

    async fn has_uid_been_settled(
        &self,
        uid: OrderUid,
        settlement_contract: &GPv2Settlement,
    ) -> Option<OrderUid> {
        let filled_amount = settlement_contract
            .filled_amount(uid.0.to_vec())
            .call()
            .await
            .unwrap_or_else(|_| U256::zero());
        // As a simplification the function is returning the uid,
        // if the order was already partially settled
        // or if it was canceled.
        if filled_amount.is_zero() {
            None
        } else {
            Some(uid)
        }
    }
}

#[async_trait::async_trait]
impl Storage for OrderBook {
    async fn add_order(&self, order: Order) -> Result<AddOrderResult> {
        let uid = order.order_meta_data.uid;
        let mut orders = self.orders.write().await;
        match orders.entry(uid) {
            Entry::Occupied(_) => return Ok(AddOrderResult::DuplicatedOrder),
            Entry::Vacant(entry) => {
                entry.insert(order);
            }
        }
        Ok(AddOrderResult::Added(uid))
    }

    async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        Ok(self
            .orders
            .read()
            .await
            .values()
            .filter(|order| {
                filter
                    .owner
                    .map(|owner| owner == order.order_meta_data.owner)
                    .unwrap_or(true)
                    && filter
                        .sell_token
                        .map(|token| token == order.order_creation.sell_token)
                        .unwrap_or(true)
                    && filter
                        .buy_token
                        .map(|token| token == order.order_creation.buy_token)
                        .unwrap_or(true)
                    && filter
                        .uid
                        .map(|uid| uid == order.order_meta_data.uid)
                        .unwrap_or(true)
            })
            .cloned()
            .collect())
    }

    #[allow(dead_code)]
    async fn remove_order(&self, uid: &OrderUid) -> Result<RemoveOrderResult> {
        let mut orders = self.orders.write().await;
        Ok(match orders.remove(uid) {
            Some(_) => RemoveOrderResult::Removed,
            None => RemoveOrderResult::DoesNotExist,
        })
    }

    // Run maintenance tasks like removing expired orders.
    async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) -> Result<()> {
        let remove_order_future = self.remove_expired_orders(now_in_epoch_seconds());
        let remove_settled_orders_future = self.remove_settled_orders(settlement_contract);
        futures::join!(remove_order_future, remove_settled_orders_future);
        tracing::info!("maintained");
        Ok(())
    }
}

#[cfg(test)]
pub mod test_util {
    use model::order::OrderCreation;

    use super::*;

    #[tokio::test]
    async fn cannot_add_order_twice() {
        let orderbook = OrderBook::default();
        let order = Order::default();
        orderbook.add_order(order.clone()).await.unwrap();
        assert_eq!(
            orderbook
                .get_orders(&OrderFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            orderbook.add_order(order).await.unwrap(),
            AddOrderResult::DuplicatedOrder
        );
    }

    #[tokio::test]
    async fn test_simple_removing_order() {
        let orderbook = OrderBook::default();
        let order = Order::default();
        let uid = match orderbook.add_order(order).await.unwrap() {
            AddOrderResult::Added(uid) => uid,
            _ => panic!("unexpected result"),
        };
        assert_eq!(
            orderbook
                .get_orders(&OrderFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        orderbook.remove_order(&uid).await.unwrap();
        assert_eq!(
            orderbook
                .get_orders(&OrderFilter::default())
                .await
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn removes_expired_orders() {
        let orderbook = OrderBook::default();
        let order = Order {
            order_creation: OrderCreation {
                valid_to: u32::MAX - 10,
                ..Default::default()
            },
            ..Default::default()
        };
        orderbook.add_order(order).await.unwrap();
        assert_eq!(
            orderbook
                .get_orders(&OrderFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        orderbook.remove_expired_orders(u32::MAX - 11).await;
        assert_eq!(
            orderbook
                .get_orders(&OrderFilter::default())
                .await
                .unwrap()
                .len(),
            1
        );
        orderbook.remove_expired_orders(u32::MAX - 9).await;
        assert_eq!(
            orderbook
                .get_orders(&OrderFilter::default())
                .await
                .unwrap()
                .len(),
            0
        );
    }
}
