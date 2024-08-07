use {super::OrderUid, crate::domain::eth};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quote {
    pub order_uid: OrderUid,
    pub sell_amount: eth::SellTokenAmount,
    pub buy_amount: eth::TokenAmount,
    pub fee: eth::SellTokenAmount,
    pub solver: eth::Address,
}
