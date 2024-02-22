use {
    crate::domain::{
        competition::{
            self,
            order::{self, Side},
        },
        eth,
    },
    std::collections::HashMap,
};

/// A trade which executes an order as part of this solution.
#[derive(Debug, Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Trade {
    Fulfillment(Fulfillment),
    Jit(Jit),
}

impl Trade {
    /// Surplus denominated in the surplus token.
    pub fn surplus(
        &self,
        prices: &HashMap<eth::TokenAddress, eth::U256>,
        weth: eth::WethAddress,
    ) -> Result<eth::Asset, Error> {
        match self {
            Self::Fulfillment(fulfillment) => {
                let prices = ClearingPrices {
                    sell: prices[&fulfillment.order().sell.token.wrap(weth)],
                    buy: prices[&fulfillment.order().buy.token.wrap(weth)],
                };

                fulfillment.surplus(prices)
            }
            // JIT orders have a zero score
            Self::Jit(jit) => Ok(eth::Asset {
                token: jit.order().sell.token,
                amount: 0.into(),
            }),
        }
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
    pub fn sell_amount(
        &self,
        prices: &HashMap<eth::TokenAddress, eth::U256>,
        weth: eth::WethAddress,
    ) -> Option<eth::TokenAmount> {
        let before_fee = match self.order.side {
            order::Side::Sell => self.executed.0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(*prices.get(&self.order.buy.token.wrap(weth))?)?
                .checked_div(*prices.get(&self.order.sell.token.wrap(weth))?)?,
        };
        Some(eth::TokenAmount(before_fee.checked_add(self.fee().0)?))
    }

    /// The effective amount the user received after all fees.
    pub fn buy_amount(
        &self,
        prices: &HashMap<eth::TokenAddress, eth::U256>,
        weth: eth::WethAddress,
    ) -> Option<eth::TokenAmount> {
        let amount = match self.order.side {
            order::Side::Buy => self.executed.0,
            order::Side::Sell => self
                .executed
                .0
                .checked_mul(*prices.get(&self.order.sell.token.wrap(weth))?)?
                .checked_div(*prices.get(&self.order.buy.token.wrap(weth))?)?,
        };
        Some(eth::TokenAmount(amount))
    }

    /// Returns the surplus denominated in the surplus token.
    ///
    /// The surplus token is a buy token for a sell order and a sell token for a
    /// buy order.
    ///
    /// The surplus is defined as the improvement of price, i.e. the difference
    /// between the executed price and the reference (limit) price.
    pub fn surplus_over_reference_price(
        &self,
        limit_sell: eth::TokenAmount,
        limit_buy: eth::TokenAmount,
        prices: ClearingPrices,
    ) -> Result<eth::U256, Error> {
        println!("limit_sell: {:?}", limit_sell);
        println!("limit_buy: {:?}", limit_buy);
        let executed = self.executed().0;
        println!("executed: {:?}", executed);
        let executed_sell_amount = match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                executed
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?
            }
            Side::Sell => executed,
        };
        println!("executed_sell_amount: {:?}", executed_sell_amount);
        // Sell slightly more `sell_token` to capture the `surplus_fee`
        let executed_sell_amount_with_fee = executed_sell_amount
            .checked_add(
                // surplus_fee is always expressed in sell token
                self.surplus_fee()
                    .map(|fee| fee.0)
                    .ok_or(Error::ProtocolFeeOnStaticOrder)?,
            )
            .ok_or(Error::Overflow)?;
        println!(
            "executed_sell_amount_with_fee: {:?}",
            executed_sell_amount_with_fee
        );
        let surplus = match self.order().side {
            Side::Buy => {
                // Scale to support partially fillable orders
                let limit_sell_amount = limit_sell
                    .0
                    .checked_mul(executed)
                    .ok_or(Error::Overflow)?
                    .checked_div(limit_buy.0)
                    .ok_or(Error::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                limit_sell_amount
                    .checked_sub(executed_sell_amount_with_fee)
                    .unwrap_or(eth::U256::zero())
            }
            Side::Sell => {
                // Scale to support partially fillable orders
                let limit_buy_amount = limit_buy
                    .0
                    .checked_mul(executed_sell_amount_with_fee)
                    .ok_or(Error::Overflow)?
                    .checked_div(limit_sell.0)
                    .ok_or(Error::DivisionByZero)?;
                // How much `buy_token` we get for `executed` amount of `sell_token`
                let executed_buy_amount = executed
                    .checked_mul(prices.sell)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.buy)
                    .ok_or(Error::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                executed_buy_amount
                    .checked_sub(limit_buy_amount)
                    .unwrap_or(eth::U256::zero())
            }
        };
        println!("surplus: {:?}", surplus);
        Ok(surplus)
    }

    /// Returns the surplus denominated in the surplus token.
    ///
    /// The surplus token is a buy token for a sell order and a sell token for a
    /// buy order.
    ///
    /// The surplus is defined as the difference between the executed price and
    /// the order limit price.
    pub fn surplus(&self, prices: ClearingPrices) -> Result<eth::Asset, Error> {
        let limit_sell = self.order().sell.amount;
        let limit_buy = self.order().buy.amount;

        self.surplus_over_reference_price(limit_sell, limit_buy, prices)
            .map(|surplus| eth::Asset {
                token: match self.order().side {
                    Side::Sell => self.order().buy.token,
                    Side::Buy => self.order().sell.token,
                },
                amount: surplus.into(),
            })
    }

    /// Returns the surplus denominated in the sell token.
    pub fn surplus_in_sell_token(
        &self,
        surplus: eth::U256,
        prices: ClearingPrices,
    ) -> Result<eth::U256, Error> {
        let surplus_in_sell_token = match self.order().side {
            Side::Buy => surplus,
            Side::Sell => surplus
                .checked_mul(prices.buy)
                .ok_or(Error::Overflow)?
                .checked_div(prices.sell)
                .ok_or(Error::DivisionByZero)?,
        };
        Ok(surplus_in_sell_token)
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
    #[error("overflow error while calculating protocol fee")]
    Overflow,
    #[error("division by zero error while calculating protocol fee")]
    DivisionByZero,
    #[error("invalid executed amount")]
    InvalidExecutedAmount,
}
