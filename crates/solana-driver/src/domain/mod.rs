//! Domain model of the Solana driver.
//!
//! These types describe the concepts the driver works with — auctions and
//! solutions — independent of any wire format or RPC representation.

pub mod auction;
pub mod order_uid;
pub mod solution;

pub use self::{
    auction::{Auction, Order, Side},
    solution::{Solution, Trade},
};
