use {
    super::ClearingPrices,
    crate::domain::{
        competition::{self, order},
        eth,
    },
    shared::conversions::U256Ext,
};

/// A trade which executes an order as part of this solution.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

/// A trade which fulfills an order from the auction.
#[derive(Debug)]
pub struct Fulfillment {
    pub order: competition::Order,
    /// The amount executed by this fulfillment. See
    /// [`competition::order::Partial`]. If the order is not partial, the
    /// executed amount must equal the amount from the order.
    pub executed: competition::order::TargetAmount,
}

/// A trade which adds a JIT order. See [`order::Jit`].
#[derive(Debug)]
pub struct Jit {
    pub order: order::Jit,
    pub executed: competition::order::TargetAmount,
}

impl Trade {
    /// Calculate the settlement contract input and output amounts executed by
    /// this trade.
    pub fn execution(&self, clearing_prices: &ClearingPrices) -> Result<Execution, Error> {
        // Values needed to calculate the executed amounts.
        let ExecutionParams {
            side,
            kind,
            sell,
            buy,
            executed,
        } = match self {
            Trade::Fulfillment(trade) => ExecutionParams {
                side: trade.order.side,
                kind: trade.order.kind,
                sell: trade.order.sell,
                buy: trade.order.buy,
                executed: trade.executed,
            },
            Trade::Jit(trade) => ExecutionParams {
                side: trade.order.side,
                // For the purposes of calculating the executed amounts, a JIT order behaves the
                // same as a regular market order.
                // TODO Martinqua said that this should be similar to liquidity and scale linearly.
                // Check if that's the default behavior for liquidity orders and of so simply use
                // that one.
                kind: order::Kind::Market,
                sell: trade.order.sell,
                buy: trade.order.buy,
                executed: trade.executed,
            },
        };

        // Clearing prices.
        let sell_price = clearing_prices
            .0
            .get(&sell.token)
            .ok_or(Error::ClearingPriceMissing)?
            .to_owned();
        let buy_price = clearing_prices
            .0
            .get(&buy.token)
            .ok_or(Error::ClearingPriceMissing)?
            .to_owned();

        // Calculate the executed amounts. For operations which require division, the
        // rounding always happens in favor of the user. Errors are returned on
        // 256-bit overflow in certain cases, even though technically they could
        // be avoided by doing BigInt conversions. The reason for this behavior is to
        // mimic the onchain settlement contract, which reverts on overflow.
        Ok(match side {
            order::Side::Buy => Execution {
                buy: eth::Asset {
                    amount: executed.into(),
                    token: buy.token,
                },
                sell: eth::Asset {
                    amount: match kind {
                        order::Kind::Market => executed
                            .0
                            .checked_mul(buy_price)
                            .ok_or(Error::Overflow)?
                            .checked_div(sell_price)
                            .ok_or(Error::Overflow)?,
                        order::Kind::Limit { .. } => todo!(),
                        order::Kind::Liquidity => todo!(),
                    },
                    token: sell.token,
                },
            },
            order::Side::Sell => Execution {
                sell: eth::Asset {
                    amount: executed.into(),
                    token: sell.token,
                },
                buy: eth::Asset {
                    amount: match kind {
                        order::Kind::Market => executed
                            .0
                            .checked_mul(sell_price)
                            .ok_or(Error::Overflow)?
                            .checked_ceil_div(&buy_price)
                            .ok_or(Error::Overflow)?,
                        order::Kind::Limit { .. } => todo!(),
                        order::Kind::Liquidity => todo!(),
                    },
                    token: buy.token,
                },
            },
        })
    }
}

/// The amounts executed by a trade.
#[derive(Debug, Clone, Copy)]
pub struct Execution {
    /// The total amount being sold.
    pub sell: eth::Asset,
    /// The total amount being bought.
    pub buy: eth::Asset,
}

/// The amounts executed by a trade.
#[derive(Debug, Clone, Copy)]
struct ExecutionParams {
    side: order::Side,
    kind: order::Kind,
    sell: eth::Asset,
    buy: eth::Asset,
    executed: order::TargetAmount,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("overflow error")]
    Overflow,
    #[error("a required clearing price was missing")]
    ClearingPriceMissing,
}
