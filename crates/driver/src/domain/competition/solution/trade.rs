use crate::{
    domain::{
        competition::{
            self,
            order::{self, FeePolicy, SellAmount, Side, TargetAmount},
            solution::error::{self, Math},
        },
        eth::{self, Asset},
    },
    util::conv::u256::U256Ext,
};

/// A trade which executes an order as part of this solution.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

impl Trade {
    pub fn side(&self) -> Side {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().side,
            Trade::Jit(jit) => jit.order().side,
        }
    }

    pub fn protocol_fees(&self) -> Vec<FeePolicy> {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().protocol_fees.to_vec(),
            Trade::Jit(_) => vec![],
        }
    }

    pub fn executed(&self) -> TargetAmount {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.executed(),
            Trade::Jit(jit) => jit.executed(),
        }
    }

    pub fn fee(&self) -> SellAmount {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.fee(),
            Trade::Jit(jit) => jit.fee,
        }
    }

    pub fn buy(&self) -> Asset {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().buy,
            Trade::Jit(jit) => jit.order().buy,
        }
    }

    pub fn sell(&self) -> Asset {
        match self {
            Trade::Fulfillment(fulfillment) => fulfillment.order().sell,
            Trade::Jit(jit) => jit.order().sell,
        }
    }

    /// The effective amount that left the user's wallet including all fees.
    fn sell_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, error::Math> {
        let before_fee = match self.side() {
            order::Side::Sell => self.executed().0,
            order::Side::Buy => self
                .executed()
                .0
                .checked_mul(prices.buy)
                .ok_or(Math::Overflow)?
                .checked_div(prices.sell)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(
            before_fee.checked_add(self.fee().0).ok_or(Math::Overflow)?,
        ))
    }

    /// The effective amount the user received after all fees.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    fn buy_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, error::Math> {
        let amount = match self.side() {
            order::Side::Buy => self.executed().0,
            order::Side::Sell => self
                .executed()
                .0
                .checked_mul(prices.sell)
                .ok_or(Math::Overflow)?
                .checked_ceil_div(&prices.buy)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(amount))
    }

    pub fn custom_prices(
        &self,
        prices: &ClearingPrices,
    ) -> Result<CustomClearingPrices, error::Math> {
        Ok(CustomClearingPrices {
            sell: self.buy_amount(prices)?.into(),
            buy: self.sell_amount(prices)?.into(),
        })
    }
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
    ) -> Result<Self, error::Trade> {
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
                    .ok_or(error::Trade::InvalidExecutedAmount)?,
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
            Err(error::Trade::InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &competition::Order {
        &self.order
    }

    pub fn side(&self) -> Side {
        self.order.side
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    /// Returns the effectively paid fee from the user's perspective
    /// considering their signed order and the uniform clearing prices
    pub fn fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => {
                // Orders with static fees are no longer used, except for quoting purposes, when
                // the static fee is set to 0. This is expected to be resolved with https://github.com/cowprotocol/services/issues/2543
                // Once resolved, this code will be simplified as part of https://github.com/cowprotocol/services/issues/2507
                order::SellAmount(0.into())
            }
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
    pub fn sell_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, error::Math> {
        let before_fee = match self.order.side {
            order::Side::Sell => self.executed.0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(prices.buy)
                .ok_or(Math::Overflow)?
                .checked_div(prices.sell)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(
            before_fee.checked_add(self.fee().0).ok_or(Math::Overflow)?,
        ))
    }

    /// The effective amount the user received after all fees.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    pub fn buy_amount(&self, prices: &ClearingPrices) -> Result<eth::TokenAmount, error::Math> {
        let amount = match self.order.side {
            order::Side::Buy => self.executed.0,
            order::Side::Sell => self
                .executed
                .0
                .checked_mul(prices.sell)
                .ok_or(Math::Overflow)?
                .checked_ceil_div(&prices.buy)
                .ok_or(Math::DivisionByZero)?,
        };
        Ok(eth::TokenAmount(amount))
    }

    pub fn custom_prices(
        &self,
        prices: &ClearingPrices,
    ) -> Result<CustomClearingPrices, error::Math> {
        Ok(CustomClearingPrices {
            sell: self.buy_amount(prices)?.into(),
            buy: self.sell_amount(prices)?.into(),
        })
    }

    /// Returns the surplus denominated in the surplus token.
    ///
    /// The surplus token is the buy token for a sell order and sell token for a
    /// buy order.
    pub fn surplus_over_reference_price(
        &self,
        limit_sell: eth::U256,
        limit_buy: eth::U256,
        prices: ClearingPrices,
    ) -> Result<eth::TokenAmount, error::Trade> {
        let executed = self.executed().0;
        let executed_sell_amount = match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                executed
                    .checked_mul(prices.buy)
                    .ok_or(Math::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Math::DivisionByZero)?
            }
            Side::Sell => executed,
        };
        // Sell slightly more `sell_token` to capture the `surplus_fee`
        let executed_sell_amount_with_fee = executed_sell_amount
            .checked_add(
                // surplus_fee is always expressed in sell token
                self.surplus_fee()
                    .map(|fee| fee.0)
                    .ok_or(error::Trade::ProtocolFeeOnStaticOrder)?,
            )
            .ok_or(Math::Overflow)?;
        let surplus = match self.order().side {
            Side::Buy => {
                // Scale to support partially fillable orders
                let limit_sell_amount = limit_sell
                    .checked_mul(executed)
                    .ok_or(Math::Overflow)?
                    .checked_div(limit_buy)
                    .ok_or(Math::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                limit_sell_amount
                    .checked_sub(executed_sell_amount_with_fee)
                    .unwrap_or(eth::U256::zero())
            }
            Side::Sell => {
                // Scale to support partially fillable orders

                // `checked_ceil_div`` to be consistent with how settlement contract calculates
                // traded buy amounts
                // smallest allowed executed_buy_amount per settlement contract is
                // executed_sell_amount * ceil(price_limits.buy / price_limits.sell)
                let limit_buy_amount = limit_buy
                    .checked_mul(executed_sell_amount_with_fee)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&limit_sell)
                    .ok_or(Math::DivisionByZero)?;
                // How much `buy_token` we get for `executed` amount of `sell_token`
                let executed_buy_amount = executed
                    .checked_mul(prices.sell)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&prices.buy)
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
/// to account for all fees (gas cost and protocol fees).
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
    fee: order::SellAmount,
}

impl Jit {
    pub fn new(
        order: order::Jit,
        executed: order::TargetAmount,
        fee: order::SellAmount,
    ) -> Result<Self, error::Trade> {
        // If the order is partial, the total executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let fee_target_amount = match order.side {
            order::Side::Buy => order::TargetAmount::default(),
            order::Side::Sell => fee.0.into(),
        };

        let executed_with_fee = order::TargetAmount(
            executed
                .0
                .checked_add(fee_target_amount.into())
                .ok_or(error::Trade::InvalidExecutedAmount)?,
        );

        // If the order is partially fillable, the executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let is_valid = match order.partially_fillable() {
            order::Partial::Yes { available } => executed_with_fee <= available,
            order::Partial::No => executed_with_fee == order.target(),
        };

        if is_valid {
            Ok(Self {
                order,
                executed,
                fee,
            })
        } else {
            Err(error::Trade::InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &order::Jit {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    pub fn fee(&self) -> order::SellAmount {
        self.fee
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
