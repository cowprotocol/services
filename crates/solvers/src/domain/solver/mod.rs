use crate::{
    domain::{auction, solution},
    infra::metrics,
};

pub mod baseline;
pub mod naive;

pub use self::{baseline::Baseline, naive::Naive};

pub enum Solver {
    Baseline(Baseline),
    Naive(Naive),
}

impl Solver {
    /// Solves a given auction and returns multiple solutions. We allow
    /// returning multiple solutions to later merge multiple non-overlapping
    /// solutions to get one big more gas efficient solution.
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        metrics::solve(&auction);
        let deadline = auction.deadline.clone();
        let solutions = match self {
            Solver::Baseline(solver) => solver.solve(auction).await,
            Solver::Naive(solver) => solver.solve(auction).await,
        };
        metrics::solved(&deadline, &solutions);
        solutions
    }
}
