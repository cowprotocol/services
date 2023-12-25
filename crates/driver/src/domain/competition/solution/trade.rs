use {
    crate::domain::{
        competition::{self, order},
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

/// A trade which fulfills an order from the auction.
#[derive(Debug, Clone)]
pub struct Fulfillment {
    order: competition::Order,
    /// The amount executed by this fulfillment. See [`order::Partial`]. If the
    /// order is not partial, the executed amount must equal the amount from the
    /// order.
    executed: order::TargetAmount,
    fee: Fee,
    protocol_fee: order::SellAmount,
}

impl Fulfillment {
    pub fn new(
        order: competition::Order,
        executed: order::TargetAmount,
        fee: Fee,
        uniform_sell_price: eth::U256,
        uniform_buy_price: eth::U256,
    ) -> Result<Self, InvalidExecutedAmount> {
        let protocol_fee = {
            let surplus_fee = match fee {
                Fee::Static => eth::U256::default(),
                Fee::Dynamic(fee) => fee.0,
            };

            let mut protocol_fee = Default::default();
            for fee_policy in &order.fee_policies {
                match fee_policy {
                    order::FeePolicy::PriceImprovement {
                        factor,
                        max_volume_factor,
                    } => {
                        let fee = match order.side {
                            order::Side::Buy => {
                                // Equal to full sell amount for FOK orders, otherwise scalled with
                                // executed amount for partially
                                // fillable orders
                                let limit_sell_amount =
                                    order.sell.amount.0 * executed.0 / order.buy.amount.0;
                                // How much `sell_token` we need to sell to buy `executed` amount of
                                // `buy_token`
                                let executed_sell_amount = executed
                                    .0
                                    .checked_mul(uniform_buy_price)
                                    .ok_or(InvalidExecutedAmount)?
                                    .checked_div(uniform_sell_price)
                                    .ok_or(InvalidExecutedAmount)?;
                                // We have to sell slightly more `sell_token` to capture the
                                // `surplus_fee`
                                let executed_sell_amount_with_surplus_fee = executed_sell_amount
                                    .checked_add(surplus_fee)
                                    .ok_or(InvalidExecutedAmount)?;
                                // Sold exactly `executed_sell_amount_with_surplus_fee` while the
                                // limit price is
                                // `limit_sell_amount` Take protocol fee from the price
                                // improvement
                                let price_improvement_fee = limit_sell_amount
                                    .checked_sub(executed_sell_amount_with_surplus_fee)
                                    .ok_or(InvalidExecutedAmount)?
                                    * (eth::U256::from_f64_lossy(factor * 100.))
                                    / 100;
                                let max_volume_fee = executed_sell_amount_with_surplus_fee
                                    * (eth::U256::from_f64_lossy(max_volume_factor * 100.))
                                    / 100;
                                // take the smaller of the two
                                std::cmp::min(price_improvement_fee, max_volume_fee)
                            }
                            order::Side::Sell => {
                                let executed_sell_amount = executed
                                    .0
                                    .checked_add(surplus_fee)
                                    .ok_or(InvalidExecutedAmount)?;

                                // Equal to full buy amount for FOK orders, otherwise scalled with
                                // executed amount for partially
                                // fillable orders
                                let limit_buy_amount =
                                    order.buy.amount.0 * executed_sell_amount / order.sell.amount.0;
                                // How much `buy_token` we get for `executed_sell_amount` of
                                // `sell_token`
                                let executed_buy_amount = executed_sell_amount
                                    .checked_mul(uniform_sell_price)
                                    .ok_or(InvalidExecutedAmount)?
                                    .checked_div(uniform_buy_price)
                                    .ok_or(InvalidExecutedAmount)?;
                                // Bought exactly `executed_buy_amount` while the limit price is
                                // `limit_buy_amount` Take protocol fee from the price
                                // improvement
                                let price_improvement_fee = executed_buy_amount
                                    .checked_sub(limit_buy_amount)
                                    .ok_or(InvalidExecutedAmount)?
                                    * (eth::U256::from_f64_lossy(factor * 100.))
                                    / 100;
                                let max_volume_fee = executed_buy_amount
                                    * (eth::U256::from_f64_lossy(max_volume_factor * 100.))
                                    / 100;
                                // take the smaller of the two
                                let protocol_fee_in_buy_amount =
                                    std::cmp::min(price_improvement_fee, max_volume_fee);

                                // express protocol fee in sell token
                                protocol_fee_in_buy_amount
                                    .checked_mul(uniform_buy_price)
                                    .ok_or(InvalidExecutedAmount)?
                                    .checked_div(uniform_sell_price)
                                    .ok_or(InvalidExecutedAmount)?
                            }
                        };
                        protocol_fee += fee;
                    }
                    order::FeePolicy::Volume { factor: _ } => unimplemented!(),
                }
            }
            order::SellAmount(protocol_fee)
        };

        // Adjust the executed amount by the protocol fee. This is because solvers are
        // unaware of the protocol fee that driver introduces and they only account
        // for the network fee.
        let executed = match order.side {
            order::Side::Buy => executed,
            order::Side::Sell => order::TargetAmount(
                executed
                    .0
                    .checked_sub(protocol_fee.0)
                    .ok_or(InvalidExecutedAmount)?,
            ),
        };

        // If the order is partial, the total executed amount can be smaller than
        // the target amount. Otherwise, the executed amount must be equal to the target
        // amount.
        let valid_execution = {
            let surplus_fee = match order.side {
                order::Side::Buy => order::TargetAmount::default(),
                order::Side::Sell => order::TargetAmount(match fee {
                    Fee::Static => eth::U256::default(),
                    Fee::Dynamic(fee) => fee.0,
                }),
            };

            let protocol_fee = match order.side {
                order::Side::Buy => order::TargetAmount::default(),
                order::Side::Sell => order::TargetAmount(protocol_fee.0),
            };

            match order.partial {
                order::Partial::Yes { available } => {
                    order::TargetAmount(executed.0 + surplus_fee.0 + protocol_fee.0) <= available
                }
                order::Partial::No => {
                    order::TargetAmount(executed.0 + surplus_fee.0 + protocol_fee.0)
                        == order.target()
                }
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
                protocol_fee,
            })
        } else {
            Err(InvalidExecutedAmount)
        }
    }

    pub fn order(&self) -> &competition::Order {
        &self.order
    }

    pub fn executed(&self) -> order::TargetAmount {
        self.executed
    }

    /// Returns the fee that should be considered as collected when
    /// scoring a solution.
    pub fn scoring_fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => self.order.fee.solver,
            Fee::Dynamic(fee) => (fee.0 + self.protocol_fee.0).into(),
        }
    }

    /// Returns the effectively paid fee from the user's perspective
    /// considering their signed order and the uniform clearing prices
    pub fn fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => self.order.fee.user,
            Fee::Dynamic(fee) => (fee.0 + self.protocol_fee.0).into(),
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
    pub fn new(
        order: order::Jit,
        executed: order::TargetAmount,
    ) -> Result<Self, InvalidExecutedAmount> {
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
            Err(InvalidExecutedAmount)
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
#[error("invalid executed amount")]
pub struct InvalidExecutedAmount;

#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    #[error("overflow error while calculating executed amounts")]
    Overflow,
    #[error("missing clearing price for {0:?}")]
    ClearingPriceMissing(eth::TokenAddress),
}
