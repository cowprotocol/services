use crate::domain::{eth, liquidity, order};
use ethereum_types::U256;
use std::collections::HashMap;

/// A solution to an auction.
pub struct Solution {
    pub prices: ClearingPrices,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
}

/// A set of uniform clearing prices. They are represented as a mapping of token
/// addresses to price in an arbitrarily denominated price.
pub struct ClearingPrices(pub HashMap<eth::TokenAddress, U256>);

/// A traded order within a solution.
pub struct Trade {
    order: order::Order,
}

impl Trade {
    /// Creates a new trade for a fully executed order.
    pub fn fill(order: order::Order) -> Trade {
        Self { order }
    }

    /// Get a reference to the traded order.
    pub fn order(&self) -> &order::Order {
        &self.order
    }

    /// Returns the trade execution as an asset (token address and amount).
    pub fn executed(&self) -> eth::Asset {
        match self.order.side {
            order::Side::Buy => self.order.buy,
            order::Side::Sell => self.order.sell,
        }
    }
}

/// A interaction included within an solution.
pub struct Interaction {
    pub liquidity: liquidity::Liquidity,
    // TODO: Currently there is not type-level guarantee that `input` and
    // output` are valid for the specified liquidity.
    pub input: eth::Asset,
    pub output: eth::Asset,
}
