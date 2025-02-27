//! DTOs modeling the HTTP REST interface of the solver.

pub mod auction;
pub mod notification;
mod solution;

pub use solution::Solutions;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
