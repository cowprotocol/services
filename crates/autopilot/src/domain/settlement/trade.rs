use crate::domain::{self, auction::order, eth};

#[derive(Debug)]
#[allow(dead_code)]
pub struct Trade {
    order_uid: domain::OrderUid,
    sell: eth::Asset,
    buy: eth::Asset,
    side: order::Side,
    executed: order::TargetAmount,
    prices: Prices,
}

impl Trade {
    pub fn new(
        order_uid: domain::OrderUid,
        sell: eth::Asset,
        buy: eth::Asset,
        side: order::Side,
        executed: order::TargetAmount,
        prices: Prices,
    ) -> Self {
        Self {
            order_uid,
            sell,
            buy,
            side,
            executed,
            prices,
        }
    }
}

#[derive(Debug)]
pub struct Prices {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}
