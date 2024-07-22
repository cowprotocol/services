//! Aggregated type, based on the mined settlement transaction.
//!
//! It contains all important information about the mined settlement, including
//! the surplus and fees.

use {
    crate::domain::{self, eth},
    std::collections::HashMap,
};

#[derive(Debug, Clone)]
pub struct Observation {
    /// The gas used by the settlement. // TODO update type
    pub gas: eth::U256,
    /// The effective gas price at the time of settlement. // TODO update type
    pub effective_gas_price: eth::U256,
    /// Total surplus expressed in native token.
    pub surplus: eth::Ether,
    /// Total fees expressed in native token.
    pub fee: eth::Ether,
    /// Per order fees denominated in sell token.
    /// Contains all orders from the settlement
    pub order_fees: HashMap<domain::OrderUid, Option<eth::Asset>>,
}
