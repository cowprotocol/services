use crate::domain::{
    auction::order::{AppDataHash, BuyTokenDestination, SellTokenSource, Side, Signature},
    eth,
    OrderUid,
};

#[derive(Clone, Debug)]
pub enum Type {
    /// A regular user order. These orders are part of the `Auction`.
    User,
    /// A regular user order that was not part of the `Auction`.
    UserOutOfAuction,
    /// A JIT order that captures surplus. These orders are usually not part of
    /// the `Auction`.
    SurplusCapturingJit,
    /// A JIT order that does not capture surplus, doesn't apply for protocol
    /// fees and is filled at it's limit prices. These orders are never part of
    /// the `Auction`.
    Jit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Jit {
    pub uid: OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: Side,
    pub valid_to: u32,
    pub fee_amount: eth::TokenAmount,
    pub receiver: eth::Address,
    pub owner: eth::Address,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub app_data: AppDataHash,
    pub signature: Signature,
    pub created: u32,
}
