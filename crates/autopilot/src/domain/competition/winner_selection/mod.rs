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

pub trait Arbitrator: Send + Sync + 'static {
    /// Picks winners and sorts all solutions where winners come before losers
    /// and higher scores come before lower scores.
    fn mark_winners(&self, solutions: Vec<Participant<Unranked>>) -> Vec<Participant>;

    /// Computes the reference scores which are used to compute
    /// rewards for the winning solvers.
    fn compute_reference_scores(&self, solutions: &[Participant]) -> HashMap<eth::Address, Score>;

    /// Removes unfair solutions from the set of all solutions.
    fn filter_solutions(
        &self,
        solutions: Vec<Participant<Unranked>>,
        auction: &Auction,
    ) -> Vec<Participant<Unranked>>;
}
