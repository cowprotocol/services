//! Minimal winner selection data structures and algorithm.
//!
//! This crate defines minimal data structures that contain only what's needed
//! to run the winner selection algorithm. Both autopilot and driver convert
//! their full solution types to these minimal structs, which are then sent to
//! the Pod Service for storage and later retrieval.
//!
//! The algorithm is generic over the chain's type vocabulary
//! ([`chain::ChainTypes`]). Every generic type defaults its chain parameter
//! to [`evm::Evm`], so EVM callers use the crate exactly as before, while
//! Solana callers instantiate the same logic with [`solana::Solana`].

pub mod arbitrator;
pub mod auction;
pub mod chain;
pub mod evm;
pub mod primitives;
pub mod solana;
pub mod solution;
pub mod state;
#[cfg(test)]
mod tests;

// Re-export key types for convenience
pub use {
    arbitrator::{Arbitrator, Ranking},
    auction::AuctionContext,
    chain::{Amount, ChainTypes},
    primitives::{Address, DirectedTokenPair, OrderUid, Side, U256},
    solution::{Order, RankType, Ranked, Scored, Solution, Unscored},
};
