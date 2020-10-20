use crate::models::Order;
use ethcontract::web3::types::Address;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

pub type OrderBookHashMap = HashMap<Address, HashMap<Address, Vec<Order>>>;

#[derive(Clone)]
pub struct OrderBook {
    pub orders: Arc<RwLock<OrderBookHashMap>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct SerializableOrderBook {
    pub orders: OrderBookHashMap,
}
impl SerializableOrderBook {
    pub fn new(orderbook: OrderBookHashMap) -> Self {
        SerializableOrderBook { orders: orderbook }
    }
}

impl Default for OrderBook {
    fn default() -> Self {
        OrderBook {
            orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}
impl OrderBook {
    pub async fn add_order(&self, order: Order) -> bool {
        let mut current_orderbook = self.orders.write().await;
        let layer_hash_map = current_orderbook.entry(order.sell_token).or_default();
        let orders = layer_hash_map.entry(order.buy_token).or_default();
        let search_result = orders.binary_search(&order);
        let pos = match search_result {
            Err(e) => e,
            Ok(_) => return false, // order is already existing
        };
        orders.insert(pos, order);
        true
    }
    #[allow(dead_code)]
    pub async fn get_orders_for_tokens(&self, token_1: &Address, token_2: &Address) -> Vec<Order> {
        let current_orderbook = self.orders.read().await;
        let empty_hash_map: HashMap<Address, Vec<Order>> = HashMap::new();
        let new_hash_map = current_orderbook.get(token_1).unwrap_or(&empty_hash_map);
        new_hash_map.get(token_2).cloned().unwrap_or_default()
    }
    #[allow(dead_code)]
    pub async fn remove_order(&self, order: Order) -> bool {
        let mut current_orderbook = self.orders.write().await;
        let layer_hash_map = current_orderbook.entry(order.sell_token).or_default();
        let orders = layer_hash_map.entry(order.buy_token).or_default();
        let search_result = orders.binary_search(&order);
        let pos = match search_result {
            Err(_) => return false, // order is not in orderbook
            Ok(e) => e,
        };
        orders.remove(pos);
        true
    }
}

#[cfg(test)]
pub mod test_util {
    use super::*;
    use ethcontract::web3::types::U256;
    #[tokio::test]
    async fn test_simple_adding_order() {
        let orderbook = OrderBook::default();
        let order = Order::new_valid_test_order();
        orderbook.add_order(order.clone()).await;
        let mut order_2 = Order::new_valid_test_order();
        order_2.sell_amount += U256::one();
        orderbook.add_order(order_2.clone()).await;

        assert_eq!(
            (orderbook.get_orders_for_tokens(&order.sell_token, &order.buy_token)).await,
            vec![order, order_2]
        );
    }
    #[tokio::test]
    async fn test_simple_removing_order() {
        let orderbook = OrderBook::default();
        let order = Order::new_valid_test_order();
        orderbook.add_order(order.clone()).await;
        let mut order_2 = Order::new_valid_test_order();
        order_2.sell_amount += U256::one();
        orderbook.add_order(order_2.clone()).await;
        orderbook.remove_order(order.clone()).await;

        assert_eq!(
            vec![order_2],
            (orderbook.get_orders_for_tokens(&order.sell_token, &order.buy_token)).await
        )
    }
    #[tokio::test]
    async fn test_no_duplication_for_adding_order() {
        let orderbook = OrderBook::default();
        let order = Order::new_valid_test_order();
        orderbook.add_order(order.clone()).await;
        let order_2 = Order::new_valid_test_order();
        assert_eq!(orderbook.add_order(order_2).await, false);

        assert_eq!(
            orderbook
                .get_orders_for_tokens(&order.sell_token, &order.buy_token)
                .await
                .len(),
            1
        );
    }
}
