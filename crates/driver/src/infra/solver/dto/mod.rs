//! DTOs modeling the HTTP REST interface of the solver.

pub mod auction;
pub mod notification;
mod solution;

pub use solution::Solutions;

/// One hundred percent expressed in basis points. Used everywhere the driver
/// converts a bps configuration value (haircut, fee policies, …) to a factor.
pub(super) const MAX_BASE_POINT: u32 = 10_000;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
