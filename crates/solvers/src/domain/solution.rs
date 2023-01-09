use crate::domain::{eth, liquidity, order};
use ethereum_types::U256;
use std::collections::HashMap;

pub struct Solution {
    pub prices: ClearingPrices,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
}

pub struct ClearingPrices(pub HashMap<eth::TokenAddress, U256>);

macro_rules! clearingprices {
    ($($args:tt)*) => {
        $crate::domain::solution::ClearingPrices(
            ::maplit::hashmap!($($args)*),
        )
    };
}

pub(crate) use clearingprices;

pub struct Trade {
    order: order::Order,
}

impl Trade {
    /// Creates a new trade for a fully executed order.
    pub fn fill(order: order::Order) -> Trade {
        Self { order }
    }

    pub fn order(&self) -> &order::Order {
        &self.order
    }

    pub fn executed(&self) -> eth::Asset {
        match self.order.side {
            order::Side::Buy => self.order.buy,
            order::Side::Sell => self.order.sell,
        }
    }
}

pub struct Interaction {
    pub liquidity: liquidity::Liquidity,
    pub input: eth::Asset,
    pub output: eth::Asset,
}
