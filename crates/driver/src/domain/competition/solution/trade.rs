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
}

impl Fulfillment {
    pub fn new(
        order: competition::Order,
        executed: order::TargetAmount,
        fee: Fee,
    ) -> Result<Self, InvalidExecutedAmount> {
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

            let executed_with_fee =
                order::TargetAmount(executed.0.checked_add(fee.0).ok_or(InvalidExecutedAmount)?);
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
            Fee::Dynamic(fee) => fee,
        }
    }

    /// Returns the effectively paid fee from the user's perspective
    /// considering their signed order and the uniform clearing prices
    pub fn fee(&self) -> order::SellAmount {
        match self.fee {
            Fee::Static => self.order.fee.user,
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
