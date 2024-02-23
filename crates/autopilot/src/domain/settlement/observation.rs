//! Aggregated type, based on the mined settlement transaction.
//!
//! It contains all important information about the mined settlement, including
//! the surplus and fees.

// TODO

use {
    crate::domain::{auction, eth, settlement},
    std::collections::HashMap,
};

pub struct Observation {
    auction: auction::Id,
    surplus: settlement::NormalizedSurplus,
    fees: settlement::Fees,
}

impl Observation {
    /// Creates a new observation from the given settlement and prices.
    ///
    /// No error handling since observation is sometimes partial but still
    /// useful. ?
    pub fn new(
        settlement: &settlement::Encoded,
        prices: &HashMap<eth::TokenAddress, auction::NormalizedPrice>,
    ) -> Self {
        Self {
            auction: settlement.auction_id(),
            surplus: settlement::Surplus::new(settlement)
                .normalized_with(prices)
                .unwrap_or_default(),
            fees: todo!(),
        }
    }
}
