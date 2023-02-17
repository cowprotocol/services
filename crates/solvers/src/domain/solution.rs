use {
    crate::domain::{eth, liquidity, order},
    ethereum_types::{Address, U256},
    std::collections::HashMap,
};

/// A solution to an auction.
pub struct Solution {
    pub prices: ClearingPrices,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
}

pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

/// A set of uniform clearing prices. They are represented as a mapping of token
/// addresses to price in an arbitrarily denominated price.
pub struct ClearingPrices(pub HashMap<eth::TokenAddress, U256>);

impl ClearingPrices {
    /// Creates a new set of clearing prices.
    pub fn new(prices: impl IntoIterator<Item = (eth::TokenAddress, U256)>) -> Self {
        Self(prices.into_iter().collect())
    }
}

/// A traded order within a solution.
pub struct Fulfillment {
    order: order::Order,
    executed: U256,
}

impl Fulfillment {
    /// Creates a new order filled to the specified amount. Returns `None` if
    /// the fill amount is incompatible with the order.
    pub fn new(order: order::Order, executed: U256) -> Option<Self> {
        let fill = match order.side {
            order::Side::Buy => order.buy.amount,
            order::Side::Sell => order.sell.amount,
        };

        if (!order.partially_fillable && executed != fill)
            || (order.partially_fillable && executed > fill)
        {
            return None;
        }

        Some(Self { order, executed })
    }

    /// Creates a new trade for a fully executed order.
    pub fn fill(order: order::Order) -> Self {
        let executed = match order.side {
            order::Side::Buy => order.buy.amount,
            order::Side::Sell => order.sell.amount,
        };
        Self { order, executed }
    }

    /// Get a reference to the traded order.
    pub fn order(&self) -> &order::Order {
        &self.order
    }

    /// Returns the trade execution as an asset (token address and amount).
    pub fn executed(&self) -> eth::Asset {
        let token = match self.order.side {
            order::Side::Buy => self.order.buy.token,
            order::Side::Sell => self.order.sell.token,
        };

        eth::Asset {
            token,
            amount: self.executed,
        }
    }
}

pub struct JitTrade {
    pub order: order::JitOrder,
    pub executed: U256,
}

/// A interaction included within an solution.
#[derive(Debug)]
pub enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

/// An interaction using input liquidity. This interaction will be encoded by
/// the driver.
#[derive(Debug)]
pub struct LiquidityInteraction {
    pub liquidity: liquidity::Liquidity,
    // TODO: Currently there is not type-level guarantee that `input` and
    // output` are valid for the specified liquidity.
    pub input: eth::Asset,
    pub output: eth::Asset,
    pub internalize: bool,
}

/// A custom interaction
#[derive(Debug)]
pub struct CustomInteraction {
    pub target: Address,
    pub value: eth::Ether,
    pub calldata: Vec<u8>,
    pub inputs: Vec<eth::Asset>,
    pub outputs: Vec<eth::Asset>,
    pub internalize: bool,
    pub allowances: Vec<Allowance>,
}

/// Approval required to make some custom interaction possible.
#[derive(Debug)]
pub struct Allowance {
    pub spender: Address,
    pub asset: eth::Asset,
}
