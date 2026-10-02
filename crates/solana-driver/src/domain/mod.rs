//! Domain model of the Solana driver.

pub mod auction;
pub mod buy_token_accounts;
pub mod competition;
pub mod order_uid;
pub mod priority_fee;
pub mod program_error;
pub mod settlement;
pub mod slot;
pub mod solution;
pub mod solver_fee;

pub use self::{
    auction::{Auction, Id, Order, Side},
    slot::Slot,
    solution::{Solution, Trade},
};
pub(crate) use self::{competition::Competition, settlement::Settlement};
