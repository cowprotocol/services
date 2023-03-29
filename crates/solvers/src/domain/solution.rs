use {
    crate::domain::{eth, liquidity, order},
    ethereum_types::{Address, U256},
    std::collections::HashMap,
};

/// A solution to an auction.
#[derive(Default)]
pub struct Solution {
    pub prices: ClearingPrices,
    pub trades: Vec<Trade>,
    pub interactions: Vec<Interaction>,
}

/// A set of uniform clearing prices. They are represented as a mapping of token
/// addresses to price in an arbitrarily denominated price.
#[derive(Default)]
pub struct ClearingPrices(pub HashMap<eth::TokenAddress, U256>);

impl ClearingPrices {
    /// Creates a new set of clearing prices.
    pub fn new(prices: impl IntoIterator<Item = (eth::TokenAddress, U256)>) -> Self {
        Self(prices.into_iter().collect())
    }
}

/// A trade which executes an order as part of this solution.
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(JitTrade),
}

/// A traded order within a solution.
pub struct Fulfillment {
    order: order::Order,
    executed: U256,
    fee: Option<U256>,
}

impl Fulfillment {
    /// Creates a new order filled to the specified amount. Returns `None` if
    /// the fill amount is incompatible with the order.
    pub fn new(order: order::Order, executed: U256, fee: Option<U256>) -> Option<Self> {
        if fee.is_some() != order.has_solver_fee() {
            return None;
        }

        let fill = match order.side {
            order::Side::Buy => order.buy.amount,
            order::Side::Sell => order.sell.amount,
        };
        let full = executed.checked_add(fee.unwrap_or_default())?;
        if (!order.partially_fillable && full != fill) || (order.partially_fillable && full > fill)
        {
            return None;
        }

        Some(Self {
            order,
            executed,
            fee,
        })
    }

    /// Creates a new trade for a fully executed order.
    pub fn fill(order: order::Order) -> Option<Self> {
        let executed = match order.side {
            order::Side::Buy => order.buy.amount,
            order::Side::Sell => order.sell.amount,
        };
        Self::new(order, executed, None)
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

    /// Returns the solver computed fee that was charged to the order as an
    /// asset (token address and amount). Returns `None` if the fulfillment
    /// does not include a solver computed fee.
    pub fn fee(&self) -> Option<eth::Asset> {
        Some(eth::Asset {
            token: self.order.sell.token,
            amount: self.fee?,
        })
    }
}

/// A trade of an order that was created specifically for this solution
/// providing just-in-time liquidity for other regular orders.
pub struct JitTrade {
    pub order: order::JitOrder,
    pub executed: U256,
}

/// An interaction that is required to execute a solution by acquiring liquidity
/// or running some custom logic.
pub enum Interaction {
    Liquidity(LiquidityInteraction),
    Custom(CustomInteraction),
}

/// An interaction using input liquidity. This interaction will be encoded by
/// the driver.
pub struct LiquidityInteraction {
    pub liquidity: liquidity::Liquidity,
    // TODO: Currently there is not type-level guarantee that `input` and
    // output` are valid for the specified liquidity.
    pub input: eth::Asset,
    pub output: eth::Asset,
    pub internalize: bool,
}

/// An arbitrary interaction returned by the solver, which needs to be executed
/// to fulfill the trade.
pub struct CustomInteraction {
    pub target: Address,
    pub value: eth::Ether,
    pub calldata: Vec<u8>,
    /// Indicated whether the interaction should be internalized (skips its
    /// execution as an optimization). This is only allowed under certain
    /// conditions.
    pub internalize: bool,
    /// Documents inputs of the interaction to determine whether internalization
    /// is actually legal.
    pub inputs: Vec<eth::Asset>,
    /// Documents outputs of the interaction to determine whether
    /// internalization is actually legal.
    pub outputs: Vec<eth::Asset>,
    /// Allowances required to successfully execute the interaction.
    pub allowances: Vec<Allowance>,
}

/// Approval required to make some `[CustomInteraction]` possible.
pub struct Allowance {
    pub spender: Address,
    pub asset: eth::Asset,
}
