//! Aggregated type, based on the mined settlement transaction.
//!
//! It contains all important information about the mined settlement, including
//! the surplus and fees.

use crate::domain::{eth, settlement};

#[derive(Debug, Clone)]
pub struct Observation {
    /// The gas used by the settlement.
    pub gas: eth::U256,
    /// The effective gas price at the time of settlement.
    pub effective_gas_price: eth::U256,
    /// Total surplus normalized to the native token (ETH).
    pub surplus: settlement::NormalizedSurplus,
    /// Total fees normalized to the native token (ETH).
    pub fee: settlement::NormalizedFee,
    /// Per order fees denominated in sell token.
    pub order_fees: settlement::Fees,
}
