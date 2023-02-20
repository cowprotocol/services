//! Core solver engine logic.

pub mod auction;
pub mod baseline;
pub mod eth;
pub mod legacy;
pub mod liquidity;
pub mod order;
pub mod solution;

pub enum Solver {
    Baseline(baseline::Baseline),
    Legacy(legacy::Legacy),
}

impl Solver {
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        match self {
            Solver::Baseline(solver) => solver.solve(auction),
            Solver::Legacy(solver) => solver.solve(auction).await,
        }
    }
}
