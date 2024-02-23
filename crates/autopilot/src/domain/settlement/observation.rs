//! Aggregated type, based on the mined settlement transaction.
//!
//! It contains all important information about the settlement, including the
//! surplus and fees.

// TODO

use crate::domain::{auction, settlement};

pub struct Observation {
    auction: auction::Id,
    surplus: settlement::NormalizedSurplus,
    fees: settlement::Fees,
}
