use {super::OrderUid, crate::domain::eth};

#[derive(Clone, Debug)]
pub struct Quote {
    pub order_uid: OrderUid,
    pub sell_amount: eth::SellTokenAmount,
    pub buy_amount: eth::TokenAmount,
    pub fee: eth::SellTokenAmount,
}
