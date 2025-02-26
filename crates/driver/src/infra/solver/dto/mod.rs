//! DTOs modeling the HTTP REST interface of the solver.

mod auction;
pub mod notification;
mod solution;

pub use {
    auction::{Auction, FlashloanHint},
    solution::Solutions,
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
