use {super::OrderUid, primitive_types::U256};

#[derive(Clone, Debug)]
pub struct Quote {
    pub order_uid: OrderUid,
    pub sell_amount: U256,
    pub buy_amount: U256,
}
