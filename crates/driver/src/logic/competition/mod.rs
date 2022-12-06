// TODO Get rid of this module
// Re-export Auction, Solution.

pub mod auction;
pub mod solution;

pub use {
    auction::Auction,
    solution::{solve, Score, Solution},
};
