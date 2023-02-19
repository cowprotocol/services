use crate::domain::{auction, solution};

pub mod balancer;
pub mod baseline;
pub mod legacy;
pub mod naive;

pub enum Solver {
    Baseline(baseline::Baseline),
    Naive(naive::Naive),
    Legacy(legacy::Legacy),
    Balancer(balancer::Balancer),
}

impl Solver {
    /// Solves a given auction and returns multiple solutions. We allow
    /// returning multiple solutions to later merge multiple non-overlapping
    /// solutions to get one big more gas efficient solution.
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        match self {
            Solver::Baseline(solver) => solver.solve(auction),
            Solver::Naive(solver) => solver.solve(auction),
            Solver::Legacy(solver) => solver.solve(auction).await,
            Solver::Balancer(solver) => solver.solve(auction).await,
        }
    }
}
