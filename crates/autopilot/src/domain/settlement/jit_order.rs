use crate::domain::{
    auction::order::{AppDataHash, BuyTokenDestination, SellTokenSource, Side, Signature},
    eth,
    OrderUid,
};

#[derive(Clone, Debug, PartialEq)]
pub struct JitOrder {
    pub uid: OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub valid_to: u32,
    pub receiver: eth::Address,
    pub owner: eth::Address,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub app_data: AppDataHash,
    pub signature: Signature,
}
