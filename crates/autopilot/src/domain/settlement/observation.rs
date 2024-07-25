//! Aggregated type containing all important information about the mined
//! settlement, including the surplus and fees.
//!
//! Observation is a snapshot of the settlement state, which purpose is to save
//! the state of the settlement to the persistence layer.

use {
    crate::domain::{self, eth},
    std::collections::HashMap,
};

#[derive(Debug, Clone)]
pub struct Observation {
    /// The gas used by the settlement.
    pub gas: eth::Gas,
    /// The effective gas price at the time of settlement.
    pub gas_price: eth::EffectiveGasPrice,
    /// Total surplus expressed in native token.
    pub surplus: eth::Ether,
    /// Total fee expressed in native token.
    pub fee: eth::Ether,
    /// Per order fees denominated in sell token. Contains all orders from the
    /// settlement
    pub order_fees: HashMap<domain::OrderUid, Option<eth::SellTokenAmount>>,
}
