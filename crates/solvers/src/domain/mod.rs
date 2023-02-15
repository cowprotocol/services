//! Core solver engine logic.

pub mod auction;
pub mod baseline;
pub mod eth;
pub mod legacy;
pub mod liquidity;
pub mod order;
pub mod solution;

pub trait Solver: Send + Sync {
    fn solve(
        &self,
        auction: auction::Auction,
    ) -> futures::future::BoxFuture<Vec<solution::Solution>>;
}
