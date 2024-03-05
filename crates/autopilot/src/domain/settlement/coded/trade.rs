use crate::domain::{
    self,
    auction::{self, order},
    eth,
    fee,
    settlement::surplus,
};

#[derive(Debug)]
pub struct Trade {
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
    pub executed: order::TargetAmount,
    pub signature: order::Signature,

    /// [ Additional derived fields ]
    ///
    /// The order uid of the order associated with this trade.
    pub order_uid: domain::OrderUid, // todo order::Uid,
    /// Derived from the settlement "clearing_prices" vector
    pub prices: Price,
}

impl Trade {
    /// [ Denominated in native token ]
    pub fn native_surplus(&self, prices: &auction::Prices) -> Result<eth::TokenAmount, Error> {
        let surplus = self.surplus_token_price(prices)?.apply(
            self.surplus()
                .ok_or(Error::Surplus(self.sell, self.buy))?
                .amount,
        );
        // normalize
        Ok((surplus.0 / eth::U256::exp10(18)).into())
    }

    /// Surplus based on uniform clearing prices returns the surplus without any
    /// fees applied.
    ///
    /// [ Denominated in surplus token ]
    fn surplus_before_fee(&self) -> Option<eth::Asset> {
        surplus::trade_surplus(
            self.side,
            self.executed,
            self.sell,
            self.buy,
            &self.prices.uniform,
        )
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// [ Denominated in surplus token ]
    pub fn surplus(&self) -> Option<eth::Asset> {
        surplus::trade_surplus(
            self.side,
            self.executed,
            self.sell,
            self.buy,
            &self.prices.custom,
        )
    }

    /// Fee is the difference between the surplus over uniform clearing prices
    /// and surplus over custom clearing prices.
    ///
    /// [ Denominated in surplus token ]
    fn fee(&self) -> Option<eth::Asset> {
        self.surplus_before_fee()
            .zip(self.surplus())
            .map(|(before, after)| eth::Asset {
                token: before.token,
                amount: before.amount.0.saturating_sub(after.amount.0).into(),
            })
    }

    /// Fee is the difference between the surplus over uniform clearing prices
    /// and surplus over custom clearing prices.
    ///
    /// [ Denominated in sell token ]
    pub fn fee_in_sell_token(&self) -> Option<eth::Asset> {
        match self.side {
            order::Side::Buy => self.fee(),
            order::Side::Sell => self.fee().map(|fee| eth::Asset {
                token: self.sell.token,
                // use uniform prices since the fee (which is determined by solvers) is expressed in
                // terms of uniform clearing prices
                amount: (fee.amount.0 * self.prices.uniform.buy / self.prices.uniform.sell).into(),
            }),
        }
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// [ Denominated in surplus token ]
    fn protocol_fee(&self, policies: &[fee::Policy]) -> Option<eth::Asset> {
        // TODO: support multiple fee policies
        if policies.len() > 1 {
            return None;
        }

        match policies.first()? {
            fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => Some(eth::Asset {
                token: match self.side {
                    order::Side::Sell => self.buy.token,
                    order::Side::Buy => self.sell.token,
                },
                amount: std::cmp::min(
                    {
                        // If the surplus after all fees is X, then the original surplus before
                        // protocol fee is X / (1 - factor)
                        apply_factor(self.surplus()?.amount.0, factor / (1.0 - factor))?
                    },
                    {
                        // Convert the executed amount to surplus token so it can be compared with
                        // the surplus
                        let executed_in_surplus_token = match self.side {
                            order::Side::Sell => {
                                self.executed.0 * self.prices.custom.sell / self.prices.custom.buy
                            }
                            order::Side::Buy => {
                                self.executed.0 * self.prices.custom.buy / self.prices.custom.sell
                            }
                        };
                        apply_factor(executed_in_surplus_token, *max_volume_factor)?
                    },
                )
                .into(),
            }),
            fee::Policy::PriceImprovement {
                factor: _,
                max_volume_factor: _,
                quote: _,
            } => todo!(),
            fee::Policy::Volume { factor: _ } => todo!(),
        }
    }

    fn surplus_token(&self) -> eth::TokenAddress {
        match self.side {
            order::Side::Buy => self.sell.token,
            order::Side::Sell => self.buy.token,
        }
    }

    /// Returns the price of the trade surplus token
    fn surplus_token_price(&self, prices: &auction::Prices) -> Result<auction::Price, Error> {
        prices
            .get(&self.surplus_token())
            .cloned()
            .ok_or(Error::MissingPrice(self.surplus_token()))
    }

    /// CIP38 score defined as surplus + protocol fee
    ///
    /// [ Denominated in surplus token ]
    pub fn score(&self, policies: &[fee::Policy]) -> Option<eth::Asset> {
        // TODO: support multiple fee policies
        if policies.len() > 1 {
            return None;
        }

        self.surplus()
            .zip(self.protocol_fee(policies))
            .map(|(surplus, fee)| eth::Asset {
                token: surplus.token,
                amount: (surplus.amount.0 + fee.amount.0).into(),
            })
    }
}

fn apply_factor(amount: eth::U256, factor: f64) -> Option<eth::U256> {
    Some(amount.checked_mul(eth::U256::from_f64_lossy(factor * 10000000000.))? / 10000000000u128)
}

#[derive(Debug)]
pub struct Price {
    pub uniform: ClearingPrices,
    /// Adjusted uniform prices to account for fees (gas cost and protocol fees)
    pub custom: ClearingPrices,
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to calculate surplus for trade sell {0:?} buy {1:?}")]
    Surplus(eth::Asset, eth::Asset),
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
}
