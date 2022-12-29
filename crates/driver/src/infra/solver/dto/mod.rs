//! DTOs modeling the HTTP REST interface of the solver.

mod auction;
mod solution;

pub use {auction::Auction, solution::Solution};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub &'static str);
