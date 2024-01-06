use {
    super::OrderUid,
    primitive_types::{H160, U256},
};

#[derive(Clone, Debug)]
pub struct Quote {
    pub order_uid: OrderUid,
    pub gas_amount: f64,
    pub gas_price: f64,
    pub sell_token_price: f64,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub solver: H160,
}
