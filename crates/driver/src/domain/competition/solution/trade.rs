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
    /// The amount executed by this JIT trade. See
    /// [`competition::order::Jit::partially_fillable`]. If the order is not
    /// partially fillable, the executed amount must equal the amount from the
    /// order.
    pub executed: competition::order::TargetAmount,
}

impl Trade {
    /// The surplus fee associated with this trade, if any.
    pub fn surplus_fee(&self) -> Option<order::SellAmount> {
        match self {
            // Surplus fees only apply to trades which fulfill limit orders.
            &Self::Fulfillment(Fulfillment {
                order:
                    competition::Order {
                        kind: order::Kind::Limit { surplus_fee },
                        ..
                    },
                ..
            }) => Some(surplus_fee),
            _ => None,
        }
    }

    /// Calculate the final sold and bought amounts that are transferred to and
    /// from the settlement contract when the settlement is executed.
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
                // same as a liquidity order. This makes sense, since their purposes are similar:
                // to make the solution better for other (market) orders.
                kind: order::Kind::Liquidity,
                sell: trade.order.sell,
                buy: trade.order.buy,
                executed: trade.executed,
            },
        };

        // For operations which require division, the rounding always happens in favor
        // of the user.
        // Errors are returned on 256-bit overflow in certain cases, even though
        // technically they could be avoided by doing BigInt conversions. The
        // reason for this behavior is to mimic the onchain settlement contract,
        // which reverts on overflow.
        Ok(match kind {
            order::Kind::Market => {
                // Market orders use clearing prices to calculate the executed amounts. See the
                // [`competition::Solution::prices`] field for an explanation of how these work.
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
                match side {
                    order::Side::Buy => Execution {
                        buy: eth::Asset {
                            amount: executed.into(),
                            token: buy.token,
                        },
                        sell: eth::Asset {
                            amount: executed
                                .0
                                .checked_mul(buy_price)
                                .ok_or(Error::Overflow)?
                                .checked_div(sell_price)
                                .ok_or(Error::Overflow)?,
                            token: sell.token,
                        },
                    },
                    order::Side::Sell => Execution {
                        sell: eth::Asset {
                            amount: executed.into(),
                            token: sell.token,
                        },
                        buy: eth::Asset {
                            amount: executed
                                .0
                                .checked_mul(sell_price)
                                .ok_or(Error::Overflow)?
                                .checked_ceil_div(&buy_price)
                                .ok_or(Error::Overflow)?,
                            token: buy.token,
                        },
                    },
                }
            }
            order::Kind::Liquidity => {
                // Liquidity orders (including JIT) compute the executed amounts by linearly
                // scaling the buy/sell amounts in the order.
                match side {
                    order::Side::Buy => Execution {
                        buy: eth::Asset {
                            amount: executed.into(),
                            token: buy.token,
                        },
                        sell: eth::Asset {
                            amount: sell
                                .amount
                                .checked_mul(executed.into())
                                .ok_or(Error::Overflow)?
                                .checked_div(buy.amount)
                                .ok_or(Error::Overflow)?,
                            token: sell.token,
                        },
                    },
                    order::Side::Sell => Execution {
                        sell: eth::Asset {
                            amount: executed.into(),
                            token: sell.token,
                        },
                        buy: eth::Asset {
                            amount: buy
                                .amount
                                .checked_mul(executed.into())
                                .ok_or(Error::Overflow)?
                                .checked_div(sell.amount)
                                .ok_or(Error::Overflow)?,
                            token: buy.token,
                        },
                    },
                }
            }
            order::Kind::Limit { surplus_fee } => {
                // Warning: calculating executed amounts for limit orders is complex and
                // confusing. To understand why the calculations work, it is important to note
                // that the solver doesn't receive limit orders with the same amounts that were
                // specified by the users when placing the orders. Instead, the limit order sell
                // amount is reduced by the surplus fee, which is the fee taken
                // by the network to settle the order. These are referred to as
                // "synthetic" limit orders. The surplus fees are calculated by the autopilot
                // when cutting the auction. This is implemented in
                // [`competition::Order::solver_sell`].
                //
                // See also [`order::Kind::Limit`].
                //
                // Similar to market orders, the executed amounts for limit orders are
                // calculated using the clearing prices.
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
                match side {
                    order::Side::Buy => Execution {
                        buy: eth::Asset {
                            amount: executed.into(),
                            token: buy.token,
                        },
                        sell: eth::Asset {
                            amount: executed
                                .0
                                .checked_mul(buy_price)
                                .ok_or(Error::Overflow)?
                                .checked_div(sell_price)
                                .ok_or(Error::Overflow)?
                                // Because of how "synthetic" limit orders are constructed as
                                // explained above, we need to simply increase the executed sell
                                // amount by the surplus fee. We know that the user placed an order
                                // big enough to cover the surplus fee.
                                .checked_add(surplus_fee.into())
                                .ok_or(Error::Overflow)?,
                            token: sell.token,
                        },
                    },
                    order::Side::Sell => Execution {
                        sell: eth::Asset {
                            amount: executed.into(),
                            token: sell.token,
                        },
                        buy: eth::Asset {
                            amount: executed
                                .0
                                // Because of how "synthetic" limit orders are constructed as
                                // explained above, the solver received the sell amount
                                // reduced by the surplus fee. That's why we have to reduce the
                                // executed amount by the surplus fee when calculating the
                                // executed buy amount.
                                .checked_sub(surplus_fee.into())
                                .ok_or(Error::Overflow)?
                                .checked_mul(sell_price)
                                .ok_or(Error::Overflow)?
                                .checked_ceil_div(&buy_price)
                                .ok_or(Error::Overflow)?,
                            token: buy.token,
                        },
                    },
                }
            }
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
