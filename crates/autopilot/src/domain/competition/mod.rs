use super::eth;

pub mod solution;

pub use solution::{Score, Solution, SolutionError, TradedAmounts, ZeroScore};

#[derive(Debug)]
pub struct Competition {
    /// The winning solver's submission address.
    pub winner: eth::Address,
    /// The winning solution's score.
    pub score: solution::Score,
    /// Deadline for an auction solution to be settled, so that it is eligible
    /// for rewards.
    pub deadline: eth::BlockNo,
}
