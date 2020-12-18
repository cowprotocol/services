use contracts::GPv2Settlement;
use futures::{future::join_all, join};
use model::{DomainSeparator, Order, OrderCreation, OrderMetaData, OrderUid};
use primitive_types::U256;
use std::{
    collections::{hash_map::Entry, HashMap},
    time::SystemTime,
};
use tokio::sync::RwLock;
use tracing::info;

#[derive(Debug, Eq, PartialEq)]
pub enum AddOrderError {
    DuplicatedOrder,
    InvalidSignature,
    #[allow(dead_code)]
    Forbidden,
    #[allow(dead_code)]
    MissingOrderData,
    #[allow(dead_code)]
    PastValidTo,
    #[allow(dead_code)]
    InsufficientFunds,
}

#[derive(Debug)]
pub enum RemoveOrderError {
    DoesNotExist,
}

#[derive(Debug, Default)]
pub struct OrderBook {
    domain_separator: DomainSeparator,
    // TODO: Store more efficiently (for example HashMap) depending on functionality we need.
    orders: RwLock<HashMap<OrderUid, Order>>,
}

impl OrderBook {
    pub fn new(domain_separator: DomainSeparator) -> Self {
        Self {
            domain_separator,
            orders: RwLock::new(HashMap::new()),
        }
    }

    pub async fn add_order(&self, order: OrderCreation) -> Result<OrderUid, AddOrderError> {
        if !has_future_valid_to(now_in_epoch_seconds(), &order) {
            return Err(AddOrderError::PastValidTo);
        }
        let order = self.order_creation_to_order(order)?;
        let uid = order.order_meta_data.uid;
        let mut orders = self.orders.write().await;
        match orders.entry(uid) {
            Entry::Occupied(_) => return Err(AddOrderError::DuplicatedOrder),
            Entry::Vacant(entry) => {
                entry.insert(order);
            }
        }
        Ok(uid)
    }

    pub async fn get_orders(&self) -> Vec<Order> {
        self.orders.read().await.values().cloned().collect()
    }

    pub async fn get_order(&self, uid: &OrderUid) -> Option<Order> {
        self.orders.read().await.get(uid).cloned()
    }

    #[allow(dead_code)]
    pub async fn remove_order(&self, uid: &OrderUid) -> Result<(), RemoveOrderError> {
        self.orders
            .write()
            .await
            .remove(uid)
            .map(|_| ())
            .ok_or(RemoveOrderError::DoesNotExist)
    }

    // Run maintenance tasks like removing expired orders.
    pub async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) {
        let remove_order_future = self.remove_expired_orders(now_in_epoch_seconds());
        let remove_settled_orders_future = self.remove_settled_orders(settlement_contract);
        join!(remove_order_future, remove_settled_orders_future);
    }

    async fn remove_expired_orders(&self, now_in_epoch_seconds: u64) {
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
        orders.retain(|uid, _| uids_to_be_removed.contains(uid));
    }

    async fn get_uids_of_settled_orders(
        &self,
        settlement_contract: &GPv2Settlement,
    ) -> Vec<OrderUid> {
        let orders = self.orders.read().await;
        let uid_futures = orders.iter().map(|(uid, _)| async move {
            self.has_uid_been_settled(*uid, settlement_contract).await
        });
        let uid_pairs: Vec<Option<OrderUid>> = join_all(uid_futures).await;
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
            .unwrap_or(U256::zero());
        // As a simplification the function is returning the uid,
        // if the order was already partially settled
        // or if it was canceled.
        if filled_amount.gt(&U256::zero()) {
            None
        } else {
            Some(uid)
        }
    }

    fn order_creation_to_order(&self, user_order: OrderCreation) -> Result<Order, AddOrderError> {
        let owner = user_order
            .validate_signature(&self.domain_separator)
            .ok_or(AddOrderError::InvalidSignature)?;
        Ok(Order {
            order_meta_data: OrderMetaData {
                creation_date: chrono::offset::Utc::now(),
                owner,
                uid: user_order.uid(&owner),
            },
            order_creation: user_order,
        })
    }
}

fn now_in_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("now earlier than epoch")
        .as_secs()
}

fn has_future_valid_to(now_in_epoch_seconds: u64, order: &OrderCreation) -> bool {
    order.valid_to as u64 > now_in_epoch_seconds
}

#[cfg(test)]
pub mod test_util {
    use super::*;

    #[tokio::test]
    async fn cannot_add_order_twice() {
        let orderbook = OrderBook::default();
        let mut order = OrderCreation::default();
        order.valid_to = u32::MAX;
        order.sign_self();
        orderbook.add_order(order).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 1);
        assert_eq!(
            orderbook.add_order(order).await,
            Err(AddOrderError::DuplicatedOrder)
        );
    }

    #[tokio::test]
    async fn test_simple_removing_order() {
        let orderbook = OrderBook::default();
        let mut order = OrderCreation::default();
        order.valid_to = u32::MAX;
        let owner = order.sign_self();
        let uid = order.uid(&owner);
        orderbook.add_order(order).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 1);
        orderbook.remove_order(&uid).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 0);
    }

    #[tokio::test]
    async fn removes_expired_orders() {
        let orderbook = OrderBook::default();
        let mut order = OrderCreation::default();
        order.valid_to = u32::MAX - 10;
        order.sign_self();
        orderbook.add_order(order).await.unwrap();
        assert_eq!(orderbook.get_orders().await.len(), 1);
        orderbook
            .remove_expired_orders((u32::MAX - 11) as u64)
            .await;
        assert_eq!(orderbook.get_orders().await.len(), 1);
        orderbook.remove_expired_orders((u32::MAX - 9) as u64).await;
        assert_eq!(orderbook.get_orders().await.len(), 0);
    }
}
