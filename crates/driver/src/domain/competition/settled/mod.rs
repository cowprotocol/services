use {
    super::{
        auction,
        order::{self, Side},
    },
    crate::{domain::eth, util::conv::u256::U256Ext},
    number::conversions::big_rational_to_u256,
};

/// Settlement in an onchain settleable form and semantics, aligned with what
/// the settlement contract expects.
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
    /// Score of a settlement is a sum of scores of all user trades in the
    /// settlement.
    ///
    /// Settlement score is valid only if all trade scores are valid.
    ///
    /// Denominated in NATIVE token
    pub fn score(&self, prices: &auction::NormalizedPrices) -> Result<eth::TokenAmount, Error> {
        self.trades
            .iter()
            .map(|trade| trade.score(prices))
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
    executed: order::TargetAmount,
    custom_price: CustomClearingPrices,
    policies: Vec<order::FeePolicy>,
}

impl Trade {
    pub fn new(
        sell: eth::Asset,
        buy: eth::Asset,
        side: Side,
        executed: order::TargetAmount,
        custom_price: CustomClearingPrices,
        policies: Vec<order::FeePolicy>,
    ) -> Self {
        Self {
            sell,
            buy,
            side,
            executed,
            custom_price,
            policies,
        }
    }

    /// CIP38 score defined as surplus + protocol fee
    ///
    /// Denominated in NATIVE token
    pub fn score(&self, prices: &auction::NormalizedPrices) -> Result<eth::TokenAmount, Error> {
        Ok(self.native_surplus(prices)? + self.native_protocol_fee(prices)?)
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
                    .checked_mul(self.executed.into())?
                    .checked_div(self.buy.amount.into())?;
                // difference between limit sell and executed amount converted to sell token
                limit_sell.checked_sub(
                    self.executed
                        .0
                        .checked_mul(self.custom_price.buy)?
                        .checked_div(self.custom_price.sell)?,
                )
            }
            Side::Sell => {
                // scale limit buy to support partially fillable orders
                let limit_buy = self
                    .executed
                    .0
                    .checked_mul(self.buy.amount.into())?
                    .checked_div(self.sell.amount.into())?;
                // difference between executed amount converted to buy token and limit buy
                self.executed
                    .0
                    .checked_mul(self.custom_price.sell)?
                    .checked_div(self.custom_price.buy)?
                    .checked_sub(limit_buy)
            }
        }
        .map(|surplus| eth::Asset {
            token: self.surplus_token(),
            amount: surplus.into(),
        })
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// Denominated in NATIVE token
    fn native_surplus(
        &self,
        prices: &auction::NormalizedPrices,
    ) -> Result<eth::TokenAmount, Error> {
        let surplus = self
            .surplus()
            .ok_or(Error::Surplus(self.sell, self.buy))?
            .amount;
        let native_price = self.surplus_token_price(prices)?;
        big_rational_to_u256(&(surplus.0.to_big_rational() * native_price.0))
            .map(Into::into)
            .map_err(Into::into)
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// Denominated in SURPLUS token
    fn protocol_fee(&self) -> Result<eth::Asset, Error> {
        // TODO: support multiple fee policies
        if self.policies.len() > 1 {
            return Err(Error::MultipleFeePolicies);
        }

        let protocol_fee = |policy: &order::FeePolicy| {
            match policy {
                order::FeePolicy::Surplus {
                    factor,
                    max_volume_factor,
                } => Ok(std::cmp::min(
                    {
                        // If the surplus after all fees is X, then the original surplus before
                        // protocol fee is X / (1 - factor)
                        let surplus = self
                            .surplus()
                            .ok_or(Error::Surplus(self.sell, self.buy))?
                            .amount;
                        apply_factor(surplus.into(), factor / (1.0 - factor))
                            .ok_or(Error::Factor(surplus.0, *factor))?
                    },
                    {
                        // Convert the executed amount to surplus token so it can be compared
                        // with the surplus
                        let executed_in_surplus_token = match self.side {
                            Side::Sell => {
                                self.executed.0 * self.custom_price.sell / self.custom_price.buy
                            }
                            Side::Buy => {
                                self.executed.0 * self.custom_price.buy / self.custom_price.sell
                            }
                        };
                        let factor = match self.side {
                            Side::Sell => max_volume_factor / (1.0 - max_volume_factor),
                            Side::Buy => max_volume_factor / (1.0 + max_volume_factor),
                        };
                        apply_factor(executed_in_surplus_token, factor)
                            .ok_or(Error::Factor(executed_in_surplus_token, factor))?
                    },
                )),
                order::FeePolicy::PriceImprovement {
                    factor: _,
                    max_volume_factor: _,
                    quote: _,
                } => Err(Error::UnsupportedFeePolicy),
                order::FeePolicy::Volume { factor: _ } => Err(Error::UnsupportedFeePolicy),
            }
        };

        let protocol_fee = self.policies.first().map(protocol_fee).transpose();
        Ok(eth::Asset {
            token: self.surplus_token(),
            amount: protocol_fee?.unwrap_or(0.into()).into(),
        })
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// Denominated in NATIVE token
    fn native_protocol_fee(
        &self,
        prices: &auction::NormalizedPrices,
    ) -> Result<eth::TokenAmount, Error> {
        let protocol_fee = self.protocol_fee()?.amount;
        let native_price = self.surplus_token_price(prices)?;
        big_rational_to_u256(&(protocol_fee.0.to_big_rational() * native_price.0))
            .map(Into::into)
            .map_err(Into::into)
    }

    fn surplus_token(&self) -> eth::TokenAddress {
        match self.side {
            Side::Buy => self.sell.token,
            Side::Sell => self.buy.token,
        }
    }

    /// Returns the normalized price of the trade surplus token
    fn surplus_token_price(
        &self,
        prices: &auction::NormalizedPrices,
    ) -> Result<auction::NormalizedPrice, Error> {
        prices
            .get(&self.surplus_token())
            .cloned()
            .ok_or(Error::MissingPrice(self.surplus_token()))
    }
}

fn apply_factor(amount: eth::U256, factor: f64) -> Option<eth::U256> {
    Some(
        amount.checked_mul(eth::U256::from_f64_lossy(factor * 1000000000000000000.))?
            / 1000000000000000000u128,
    )
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

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("multiple fee policies are not supported yet")]
    MultipleFeePolicies,
    #[error("failed to calculate surplus for trade sell {0:?} buy {1:?}")]
    Surplus(eth::Asset, eth::Asset),
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error("type conversion error")]
    TypeConversion(#[from] anyhow::Error),
    #[error("fee policy not supported")]
    UnsupportedFeePolicy,
    #[error("factor {1} multiplication with {0} failed")]
    Factor(eth::U256, f64),
}
