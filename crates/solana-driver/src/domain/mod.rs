//! Domain model of the Solana driver.

pub mod auction;
pub mod competition;
pub mod order_uid;
pub mod slot;
pub mod solution;

pub use self::{
    auction::{Auction, Id, Order, Side},
    competition::Competition,
    slot::Slot,
    solution::{Solution, Trade},
};
