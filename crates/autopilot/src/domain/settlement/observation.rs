//! Aggregated type, based on the mined settlement transaction.
//!
//! It contains all important information about the mined settlement, including
//! the surplus and fees.

use {
    crate::domain::{auction::order, eth},
    std::collections::HashMap,
};

#[derive(Debug, Clone)]
pub struct Observation {
    /// The gas used by the settlement.
    pub gas: eth::U256,
    /// The effective gas price at the time of settlement.
    pub effective_gas_price: eth::U256,
    /// Total surplus expressed in native token.
    pub surplus: eth::TokenAmount,
    /// Total fees expressed in native token.
    pub fee: eth::TokenAmount,
    /// Per order fees denominated in sell token.
    /// Contains all orders from the settlement
    pub order_fees: HashMap<order::OrderUid, Option<eth::Asset>>,
}
