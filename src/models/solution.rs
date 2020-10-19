use crate::models::Order;

#[derive(Debug)]
pub struct Solution {
    pub sell_orders_token0: Vec<Order>,
    pub sell_orders_token1: Vec<Order>,
}
