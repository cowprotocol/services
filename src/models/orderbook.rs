use crate::models::Order;
use anyhow::Result;
use ethcontract::web3::types::Address;
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

pub type OrderBookHashMap = HashMap<Address, HashMap<Address, Vec<Order>>>;

#[derive(Clone, Deserialize)]
pub struct OrderBook {
    #[serde(with = "arc_rwlock_serde")]
    pub orders: Arc<RwLock<OrderBookHashMap>>,
}

mod arc_rwlock_serde {
    use parking_lot::RwLock;
    use serde::de::Deserializer;
    use serde::Deserialize;
    use std::sync::Arc;

    pub fn deserialize<'de, D, T>(d: D) -> Result<Arc<RwLock<T>>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        Ok(Arc::new(RwLock::new(T::deserialize(d)?)))
    }
}

impl OrderBook {
    #[allow(dead_code)]
    pub fn new() -> Self {
        OrderBook {
            orders: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    #[allow(dead_code)]
    pub fn add_order(&mut self, order: Order) -> bool {
        let mut current_orderbook = self.orders.write();
        let layer_hash_map = current_orderbook.entry(order.sell_token).or_default();
        let orders = layer_hash_map.entry(order.buy_token).or_default();
        let search_result = orders.binary_search(&order.clone());
        let pos = match search_result {
            Err(e) => e,
            Ok(_) => return false, // order is already existing
        };
        orders.insert(pos, order.clone());
        true
    }
    #[allow(dead_code)]
    pub fn get_orders_for_tokens(self, token_1: Address, token_2: Address) -> Result<Vec<Order>> {
        let current_orderbook = self.orders.read();
        let empty_hash_vec: Vec<Order> = Vec::new();
        let empty_hash_map: HashMap<Address, Vec<Order>> = HashMap::new();
        let new_hash_map = current_orderbook.get(&token_1).unwrap_or(&empty_hash_map);
        Ok(new_hash_map
            .get(&token_2)
            .cloned()
            .unwrap_or(empty_hash_vec))
    }
    #[allow(dead_code)]
    pub fn remove_order(&mut self, order: Order) -> bool {
        let mut current_orderbook = self.orders.write();
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

    #[test]
    fn test_simple_adding_order() {
        let mut orderbook = OrderBook::new();
        let order = Order::new_valid_test_order();
        orderbook.add_order(order.clone());
        let mut order_2 = Order::new_valid_test_order();
        order_2.sell_amount = order_2.sell_amount + U256::one();
        orderbook.add_order(order_2.clone());

        assert_eq!(
            (orderbook.get_orders_for_tokens(order.sell_token, order.buy_token)).unwrap(),
            vec![order, order_2]
        );
    }
    #[test]
    fn test_simple_removing_order() {
        let mut orderbook = OrderBook::new();
        let order = Order::new_valid_test_order();
        orderbook.add_order(order.clone());
        let mut order_2 = Order::new_valid_test_order();
        order_2.sell_amount = order_2.sell_amount + U256::one();
        orderbook.add_order(order_2.clone());
        orderbook.remove_order(order.clone());

        assert_eq!(
            vec![order_2],
            (orderbook.get_orders_for_tokens(order.sell_token, order.buy_token)).unwrap()
        )
    }
    #[test]
    fn test_no_duplication_for_adding_order() {
        let mut orderbook = OrderBook::new();
        let order = Order::new_valid_test_order();
        orderbook.add_order(order.clone());
        let order_2 = Order::new_valid_test_order();
        assert_eq!(orderbook.add_order(order_2), false);

        assert_eq!(
            orderbook
                .get_orders_for_tokens(order.sell_token, order.buy_token)
                .unwrap()
                .len(),
            1
        );
    }
}
