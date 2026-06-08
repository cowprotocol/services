//! DTOs modeling the HTTP REST interface of the solver.

use crate::domain::competition::order::{
    self,
    fees::{FeePolicy, ProtocolFee},
};

pub mod auction;
pub mod notification;
mod solution;

pub use solution::Solutions;

/// One hundred percent expressed in basis points. Used everywhere the driver
/// converts a bps configuration value (haircut, fee policies, …) to a factor.
pub(super) const MAX_BASE_POINT: u32 = 10_000;

/// Builds the synthetic non-scoring fee that represents a solver's haircut.
///
/// The haircut is a per-solver conservative-bidding buffer. We model it as a
/// volume fee with `contributes_to_score == false` so it reuses the volume-fee
/// make-room and post-processing while staying out of the score and out of
/// revenue accounting. Returns `None` when no haircut is configured.
pub(super) fn haircut_fee(haircut_bps: u32) -> Option<ProtocolFee> {
    (haircut_bps > 0).then(|| {
        order::ProtocolFee::non_scoring(FeePolicy::Volume {
            factor: f64::from(haircut_bps) / f64::from(MAX_BASE_POINT),
        })
    })
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct Error(pub String);
