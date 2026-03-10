pub mod baseline;
pub mod dex;

pub use self::{
    baseline::{Config, Request, Route, Segment, Solver as Baseline},
    dex::Dex,
};
use crate::domain::{auction, solution};

pub enum Solver {
    Baseline(Baseline),
    Dex(Box<Dex>),
}

impl Solver {
    pub async fn solve(&self, auction: auction::Auction) -> Vec<solution::Solution> {
        match self {
            Solver::Baseline(s) => s.solve(auction).await,
            Solver::Dex(s) => s.solve(auction).await,
        }
    }
}
