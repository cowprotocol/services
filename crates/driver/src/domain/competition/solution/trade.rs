use {
    super::error::Math,
    crate::domain::{
        competition::{
            self,
            order::{self, Side},
        },
        eth,
    },
};

/// A trade which executes an order as part of this solution.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

/// A trade which fulfills an order from the auction.
#[derive(Debug, Clone)]
pub struct Fulfillment {
    order: competition::Order,
    /// The amount executed by this fulfillment. See [`order::Partial`]. If the
    /// order is not partial, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
    fee: Fee,
}

impl Fulfillment {
    pub fn new(
        order: competition::Order,
        executed: order::TargetAmount,
        fee: Fee,
    ) -> Result<Self, Error> {
        // If the order is partial, the total executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let valid_execution = {
            let fee = match order.side {
                order::Side::Buy => order::TargetAmount::default(),
                order::Side::Sell => order::TargetAmount(match fee {
                    Fee::Static => eth::U256::default(),
                    Fee::Dynamic(fee) => fee.0,
                }),
            };

            let executed_with_fee = order::TargetAmount(
                executed
                    .0
                    .checked_add(fee.0)
                    .ok_or(Error::InvalidExecutedAmount)?,
            );
            match order.partial {
                order::Partial::Yes { available } => executed_with_fee <= available,
                order::Partial::No => executed_with_fee == order.target(),
            }
        };

        // Only accept solver-computed fees if the order requires them, otherwise the
        // protocol pre-determines the fee and the solver must respect it.
        let valid_fee = match &fee {
            Fee::Static => !order.solver_determines_fee(),
            Fee::Dynamic(_) => order.solver_determines_fee(),
        };

        if valid_execution && valid_fee {
            Ok(Self {
                order,
                executed,
                fee,
            })
        } else {
            Err(Error::InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &competition::Order {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    /// Returns the effectively paid fee from the user's perspective
    /// considering their signed order and the uniform clearing prices
    pub fn fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => self.order.user_fee,
            Fee::Dynamic(fee) => fee,
        }
    }

    /// Returns the solver determined fee if it exists.
    pub fn surplus_fee(&self) -> Option<order::SellAmount> {
        match self.fee {
            Fee::Static => None,
            Fee::Dynamic(fee) => Some(fee),
        }
    }

    /// The effective amount that left the user's wallet including all fees.
    pub fn sell_amount(&self, prices: &CustomClearingPrices) -> Result<eth::TokenAmount, Error> {
        let amount = match self.order.side {
            order::Side::Sell => self.executed.0 + self.fee().0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(prices.sell)
                .ok_or(Math::Overflow)?
                .checked_div(prices.buy)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(amount))
    }

    /// The effective amount the user received after all fees.
    pub fn buy_amount(&self, prices: &CustomClearingPrices) -> Result<eth::TokenAmount, Error> {
        let amount = match self.order.side {
            order::Side::Buy => self.executed.0,
            order::Side::Sell => (self.executed.0 + self.fee().0)
                .checked_mul(prices.sell)
                .ok_or(Math::Overflow)?
                .checked_div(prices.buy)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(amount))
    }

    /// Returns the adjusted clearing prices which account for the fee.
    pub fn custom_prices(&self, uniform: &ClearingPrices) -> Result<CustomClearingPrices, Math> {
        let custom_prices = CustomClearingPrices {
            sell: match self.order.side {
                order::Side::Sell => self
                    .executed
                    .0
                    .checked_mul(uniform.sell)
                    .ok_or(Math::Overflow)?
                    .checked_div(uniform.buy)
                    .ok_or(Math::DivisionByZero)?,
                order::Side::Buy => self.executed.0,
            },
            buy: match self.order.side {
                order::Side::Sell => self.executed.0 + self.fee().0,
                order::Side::Buy => {
                    (self.executed.0)
                        .checked_mul(uniform.buy)
                        .ok_or(Math::Overflow)?
                        .checked_div(uniform.sell)
                        .ok_or(Math::DivisionByZero)?
                        + self.fee().0
                }
            },
        };
        Ok(custom_prices)
    }

    /// Returns the surplus denominated in the surplus token.
    ///
    /// The surplus token is the buy token for a sell order and sell token for a
    /// buy order.
    pub fn surplus_over_reference_price(
        &self,
        limit_sell: eth::U256,
        limit_buy: eth::U256,
        prices: &CustomClearingPrices,
    ) -> Result<eth::TokenAmount, Error> {
        let sell_amount = self.sell_amount(prices)?;
        let buy_amount = self.buy_amount(prices)?;
        let surplus = match self.order().side {
            Side::Buy => {
                // Scale to support partially fillable orders
                let limit_sell_amount = limit_sell
                    .checked_mul(buy_amount.0)
                    .ok_or(Math::Overflow)?
                    .checked_div(limit_buy)
                    .ok_or(Math::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                limit_sell_amount
                    .checked_sub(sell_amount.0)
                    .unwrap_or(eth::U256::zero())
            }
            Side::Sell => {
                // Scale to support partially fillable orders
                let limit_buy_amount = limit_buy
                    .checked_mul(sell_amount.0)
                    .ok_or(Math::Overflow)?
                    .checked_div(limit_sell)
                    .ok_or(Math::DivisionByZero)?;
                // How much `buy_token` we get for `executed` amount of `sell_token`
                let executed_buy_amount = sell_amount
                    .0
                    .checked_mul(prices.sell)
                    .ok_or(Math::Overflow)?
                    .checked_div(prices.buy)
                    .ok_or(Math::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                executed_buy_amount
                    .checked_sub(limit_buy_amount)
                    .unwrap_or(eth::U256::zero())
            }
        };
        Ok(surplus.into())
    }
}

/// A fee that is charged for executing an order.
#[derive(Clone, Copy, Debug)]
pub enum Fee {
    /// A static protocol computed fee.
    ///
    /// That is, the fee is known upfront and is signed as part of the order
    Static,
    /// A dynamic solver computed surplus fee.
    Dynamic(order::SellAmount),
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

/// Custom clearing prices at which the trade was executed.
///
/// These prices differ from uniform clearing prices, in that they are adjusted
/// to account for fee.
///
/// These prices determine the actual traded amounts from the user perspective.
#[derive(Debug, Clone)]
pub struct CustomClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

/// A trade which adds a JIT order. See [`order::Jit`].
#[derive(Debug, Clone)]
pub struct Jit {
    order: order::Jit,
    /// The amount executed by this JIT trade. See
    /// [`order::Jit::partially_fillable`]. If the order is not
    /// partially fillable, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
}

impl Jit {
    pub fn new(order: order::Jit, executed: order::TargetAmount) -> Result<Self, Error> {
        // If the order is partially fillable, the executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let is_valid = if order.partially_fillable {
            executed <= order.target()
        } else {
            executed == order.target()
        };
        if is_valid {
            Ok(Self { order, executed })
        } else {
            Err(Error::InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &order::Jit {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("orders with non solver determined gas cost fees are not supported")]
    ProtocolFeeOnStaticOrder,
    #[error("invalid executed amount")]
    InvalidExecutedAmount,
    #[error(transparent)]
    Math(#[from] Math),
}
