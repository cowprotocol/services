use std::collections::HashMap;

pub mod auction;
pub mod fee;
pub mod quote;

pub use {
    auction::{
        order::{Order, OrderUid},
        Auction,
        AuctionId,
        AuctionWithId,
    },
    fee::ProtocolFee,
    quote::Quote,
};

pub struct SolvableOrders {
    pub orders: Vec<model::order::Order>,
    pub quotes: HashMap<OrderUid, Quote>,
    pub latest_settlement_block: u64,
}
