//! Minimal winner selection data structures and algorithm.
//!
//! This crate defines minimal data structures that contain only what's needed
//! to run the winner selection algorithm. Both autopilot and driver convert
//! their full solution types to these minimal structs, which are then sent to
//! the Pod Service for storage and later retrieval.

pub mod arbitrator;
pub mod auction;
pub mod primitives;
pub mod solution;
pub mod util;

// Re-export key types for convenience
pub use {
    arbitrator::{Arbitrator, Ranking},
    auction::AuctionContext,
    primitives::{
        DirectedTokenPair,
        Ether,
        OrderUid,
        Price,
        Score,
        Side,
        TokenAddress,
        TokenAmount,
        WrappedNativeToken,
    },
    solution::{Order, Solution},
};
