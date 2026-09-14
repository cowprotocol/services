//! Domain model of the Solana driver.

pub mod auction;
pub mod competition;
pub mod order_uid;
pub mod settlement;
pub mod slot;
pub mod solution;

pub use self::{
    auction::{Auction, Id, Order, Side},
    slot::Slot,
    solution::{Solution, Trade},
};
pub(crate) use self::{competition::Competition, settlement::Settlement};
