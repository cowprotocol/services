use {
    super::{
        auction,
        order::{self, Side},
    },
    crate::{domain::eth, util::conv::u256::U256Ext},
    number::conversions::big_rational_to_u256,
};

/// Settlement built from onchain calldata.
#[derive(Debug, Clone)]
pub struct Settlement {
    trades: Vec<Trade>,
}

impl Settlement {
    pub fn new(trades: Vec<Trade>) -> Self {
        Self { trades }
    }

    /// Score of a settlement as per CIP38
    ///
    /// Denominated in NATIVE token
    pub fn score(&self) -> Option<eth::TokenAmount> {
        self.trades
            .iter()
            .map(Trade::score)
            .try_fold(eth::TokenAmount(eth::U256::zero()), |acc, score| {
                score.map(|score| acc + score)
            })
    }
}

#[derive(Debug, Clone)]
pub struct Trade {
    sell: eth::Asset,
    buy: eth::Asset,
    side: Side,
    executed: eth::Asset,
    prices: Prices,
    policies: Vec<order::FeePolicy>,
}

impl Trade {
    pub fn new(
        sell: eth::Asset,
        buy: eth::Asset,
        side: Side,
        executed: eth::Asset,
        prices: Prices,
        policies: Vec<order::FeePolicy>,
    ) -> Self {
        Self {
            sell,
            buy,
            side,
            executed,
            prices,
            policies,
        }
    }

    /// CIP38 score defined as surplus + protocol fee
    ///
    /// Denominated in NATIVE token
    pub fn score(&self) -> Option<eth::TokenAmount> {
        self.native_surplus()
            .zip(self.native_protocol_fee())
            .map(|(surplus, fee)| surplus + fee)
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// Denominated in SURPLUS token
    fn surplus(&self) -> Option<eth::Asset> {
        match self.side {
            Side::Buy => {
                // scale limit sell to support partially fillable orders
                let limit_sell = self
                    .sell
                    .amount
                    .0
                    .checked_mul(self.executed.amount.into())?
                    .checked_div(self.buy.amount.into())?;
                // difference between limit sell and executed amount converted to sell token
                limit_sell.checked_sub(
                    self.executed
                        .amount
                        .0
                        .checked_mul(self.prices.custom.buy)?
                        .checked_div(self.prices.custom.sell)?,
                )
            }
            Side::Sell => {
                // scale limit buy to support partially fillable orders
                let limit_buy = self
                    .executed
                    .amount
                    .0
                    .checked_mul(self.buy.amount.into())?
                    .checked_div(self.sell.amount.into())?;
                // difference between executed amount converted to buy token and limit buy
                self.executed
                    .amount
                    .0
                    .checked_mul(self.prices.custom.sell)?
                    .checked_div(self.prices.custom.buy)?
                    .checked_sub(limit_buy)
            }
        }
        .map(|surplus| match self.side {
            Side::Buy => eth::Asset {
                amount: surplus.into(),
                token: self.sell.token,
            },
            Side::Sell => eth::Asset {
                amount: surplus.into(),
                token: self.buy.token,
            },
        })
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// Denominated in NATIVE token
    fn native_surplus(&self) -> Option<eth::TokenAmount> {
        big_rational_to_u256(
            &(self.surplus()?.amount.0.to_big_rational() * self.surplus_token_price().0),
        )
        .map(Into::into)
        .ok()
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// Denominated in SURPLUS token
    fn protocol_fee(&self) -> Option<eth::Asset> {
        // TODO: support multiple fee policies
        if self.policies.len() > 1 {
            return None;
        }

        match self.policies.first()? {
            order::FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => Some(eth::Asset {
                token: match self.side {
                    Side::Sell => self.buy.token,
                    Side::Buy => self.sell.token,
                },
                amount: std::cmp::min(
                    {
                        // If the surplus after all fees is X, then the original
                        // surplus before protocol fee is X / (1 - factor)
                        apply_factor(self.surplus()?.amount.into(), factor / (1.0 - factor))?
                    },
                    {
                        // Convert the executed amount to surplus token so it can be compared with
                        // the surplus
                        let executed_in_surplus_token = match self.side {
                            Side::Sell => {
                                self.executed.amount.0 * self.prices.custom.sell
                                    / self.prices.custom.buy
                            }
                            Side::Buy => {
                                self.executed.amount.0 * self.prices.custom.buy
                                    / self.prices.custom.sell
                            }
                        };
                        apply_factor(
                            executed_in_surplus_token,
                            match self.side {
                                Side::Sell => max_volume_factor / (1.0 - max_volume_factor),
                                Side::Buy => max_volume_factor / (1.0 + max_volume_factor),
                            },
                        )?
                    },
                )
                .into(),
            }),
            order::FeePolicy::PriceImprovement {
                factor: _,
                max_volume_factor: _,
                quote: _,
            } => todo!(),
            order::FeePolicy::Volume { factor: _ } => todo!(),
        }
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// Denominated in NATIVE token
    fn native_protocol_fee(&self) -> Option<eth::TokenAmount> {
        big_rational_to_u256(
            &(self.protocol_fee()?.amount.0.to_big_rational() * self.surplus_token_price().0),
        )
        .map(Into::into)
        .ok()
    }

    /// Returns the normalized price of the trade surplus token
    fn surplus_token_price(&self) -> auction::NormalizedPrice {
        match self.side {
            Side::Buy => self.prices.native.sell.clone(),
            Side::Sell => self.prices.native.buy.clone(),
        }
    }
}

fn apply_factor(amount: eth::U256, factor: f64) -> Option<eth::U256> {
    Some(
        amount.checked_mul(eth::U256::from_f64_lossy(factor * 1000000000000000000.))?
            / 1000000000000000000u128,
    )
}

#[derive(Debug, Clone)]
pub struct Prices {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
    /// Prices normalized to the same native token (ETH)
    pub native: NormalizedPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

/// Normalized prices to the same native token (ETH)
#[derive(Debug, Clone)]
pub struct NormalizedPrices {
    pub sell: auction::NormalizedPrice,
    pub buy: auction::NormalizedPrice,
}
