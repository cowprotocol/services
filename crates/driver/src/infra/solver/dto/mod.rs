//! DTOs modeling the HTTP REST interface of the solver.

mod auction;
mod notification;
mod solution;

pub use {
    auction::Auction,
    notification::Notification,
    solution::{FlashloanLender, Solutions},
};

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
