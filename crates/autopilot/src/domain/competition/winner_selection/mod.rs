use {
    crate::domain::{
        Auction,
        competition::{Participant, Score, Unranked},
        eth,
    },
    std::collections::HashMap,
};

pub mod combinatorial;
pub mod max_score;

/// Implements auction arbitration in 3 phases:
/// 1. filter unfair solutions
/// 2. mark winners
/// 3. compute reference scores
///
/// The functions assume the `Arbitrator` is the only one
/// changing the ordering or the `participants.
pub trait Arbitrator: Send + Sync + 'static {
    /// Removes unfair solutions from the set of all solutions.
    fn filter_unfair_solutions(
        &self,
        participants: Vec<Participant<Unranked>>,
        auction: &Auction,
    ) -> Vec<Participant<Unranked>>;

    /// Picks winners and sorts all solutions where winners come before losers
    /// and higher scores come before lower scores.
    fn mark_winners(&self, participants: Vec<Participant<Unranked>>) -> Vec<Participant>;

    /// Computes the reference scores which are used to compute
    /// rewards for the winning solvers.
    fn compute_reference_scores(
        &self,
        participants: &[Participant],
    ) -> HashMap<eth::Address, Score>;
}
