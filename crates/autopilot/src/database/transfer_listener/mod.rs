//! Listens for ERC20 Transfer events and cancels orders that have transferred
//! their sell tokens away from the order owner.

pub mod listener;

pub use listener::TransferListener;
