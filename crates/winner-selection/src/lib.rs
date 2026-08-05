//! Minimal winner selection data structures and algorithm.
//!
//! This crate defines minimal data structures that contain only what's needed
//! to run the winner selection algorithm. Both autopilot and driver convert
//! their full solution types to these minimal structs, which are then sent to
//! the Pod Service for storage and later retrieval.
//!
//! The algorithm is generic over the chain's type vocabulary
//! ([`chain::ChainTypes`]). Every generic type defaults its chain parameter
//! to [`evm::Evm`], so bare type names mean the EVM instantiation, and
//! Solana callers instantiate the same logic with [`solana::Solana`].

pub mod arbitrator;
pub mod auction;
pub mod primitives;
pub mod solution;
pub mod state;

pub use chain_types::{self as chain, evm, solana};
#[cfg(test)]
mod tests;

// Re-export key types for convenience
pub use {
    arbitrator::{Arbitrator, Ranking},
    auction::AuctionContext,
    chain_types::{Amount, ChainTypes},
    primitives::{Address, DirectedTokenPair, OrderUid, Side, U256},
    solution::{Order, RankType, Ranked, Scored, Solution, Unscored},
};
