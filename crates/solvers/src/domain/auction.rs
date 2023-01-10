//! The domain object representing an auction.

use crate::domain::{liquidity, order};

/// A domain model for an auction to solve.
pub struct Auction {
    pub orders: Vec<order::Order>,
    pub liquidity: Vec<liquidity::Liquidity>,
}
