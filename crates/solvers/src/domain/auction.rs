//! The domain object representing an auction.

use crate::domain::{liquidity, order};

pub struct Auction {
    pub orders: Vec<order::Order>,
    pub liquidity: Vec<liquidity::Liquidity>,
}
