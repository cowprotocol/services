use crate::domain::{
    self,
    auction::{self, order},
    eth,
    fee,
};

#[derive(Debug)]
pub struct Trade {
    order_uid: domain::OrderUid, // todo order::Uid
    sell: eth::Asset,
    buy: eth::Asset,
    side: order::Side,
    executed: order::TargetAmount,
    prices: Prices,
}

impl Trade {
    pub fn new(
        order_uid: domain::OrderUid,
        sell: eth::Asset,
        buy: eth::Asset,
        side: order::Side,
        executed: order::TargetAmount,
        prices: Prices,
    ) -> Self {
        Self {
            order_uid,
            sell,
            buy,
            side,
            executed,
            prices,
        }
    }

    pub fn order_uid(&self) -> domain::OrderUid {
        self.order_uid
    }

    /// CIP38 score defined as surplus + protocol fee
    ///
    /// Denominated in NATIVE token
    pub fn score(
        &self,
        prices: &auction::Prices,
        policies: &[fee::Policy],
    ) -> Result<eth::TokenAmount, Error> {
        Ok(self.native_surplus(prices)? + self.native_protocol_fee(prices, policies)?)
    }

    /// Denominated in NATIVE token
    pub fn native_surplus(&self, prices: &auction::Prices) -> Result<eth::TokenAmount, Error> {
        let surplus = self.surplus_token_price(prices)?.apply(
            self.surplus()
                .ok_or(Error::Surplus(self.sell, self.buy))?
                .amount,
        );
        // normalize
        Ok((surplus.0 / eth::U256::exp10(18)).into())
    }

    /// Fee is the difference between the surplus over uniform clearing prices
    /// and surplus over custom clearing prices.
    ///
    /// Denominated in NATIVE token
    pub fn native_fee(&self, prices: &auction::Prices) -> Result<eth::TokenAmount, Error> {
        let fee = self
            .surplus_token_price(prices)?
            .apply(self.fee_in_sell_token()?.amount);
        // normalize
        Ok((fee.0 / eth::U256::exp10(18)).into())
    }

    /// Surplus based on uniform clearing prices returns the surplus without any
    /// fees applied.
    ///
    /// Denominated in SURPLUS token
    fn surplus_before_fee(&self) -> Option<eth::Asset> {
        trade_surplus(
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
    /// Denominated in SURPLUS token
    pub fn surplus(&self) -> Option<eth::Asset> {
        trade_surplus(
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
    /// Denominated in SURPLUS token
    fn fee(&self) -> Result<eth::Asset, Error> {
        self.surplus_before_fee()
            .zip(self.surplus())
            .map(|(before, after)| eth::Asset {
                token: before.token,
                amount: before.amount.0.saturating_sub(after.amount.0).into(),
            })
            .ok_or(Error::Surplus(self.sell, self.buy))
    }

    /// Fee is the difference between the surplus over uniform clearing prices
    /// and surplus over custom clearing prices.
    ///
    /// Denominated in SELL token
    pub fn fee_in_sell_token(&self) -> Result<eth::Asset, Error> {
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
    /// Denominated in SURPLUS token
    fn protocol_fee(&self, policies: &[fee::Policy]) -> Result<eth::Asset, Error> {
        // TODO: support multiple fee policies
        if policies.len() > 1 {
            return Err(Error::MultipleFeePolicies);
        }

        let protocol_fee = |policy: &fee::Policy| {
            match policy {
                fee::Policy::Surplus {
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
                            order::Side::Sell => self
                                .executed
                                .0
                                .checked_mul(self.prices.custom.sell)
                                .ok_or(MathError::Overflow)?
                                .checked_div(self.prices.custom.buy)
                                .ok_or(MathError::DivisionByZero)?,
                            order::Side::Buy => self
                                .executed
                                .0
                                .checked_mul(self.prices.custom.buy)
                                .ok_or(MathError::Overflow)?
                                .checked_div(self.prices.custom.sell)
                                .ok_or(MathError::DivisionByZero)?,
                        };
                        let factor = match self.side {
                            order::Side::Sell => max_volume_factor / (1.0 - max_volume_factor),
                            order::Side::Buy => max_volume_factor / (1.0 + max_volume_factor),
                        };
                        apply_factor(executed_in_surplus_token, factor)
                            .ok_or(Error::Factor(executed_in_surplus_token, factor))?
                    },
                )),
                fee::Policy::PriceImprovement {
                    factor: _,
                    max_volume_factor: _,
                    quote: _,
                } => Err(Error::UnimplementedFeePolicy),
                fee::Policy::Volume { factor: _ } => Err(Error::UnimplementedFeePolicy),
            }
        };

        let protocol_fee = policies.first().map(protocol_fee).transpose();
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
        prices: &auction::Prices,
        policies: &[fee::Policy],
    ) -> Result<eth::TokenAmount, Error> {
        let protocol_fee = self
            .surplus_token_price(prices)?
            .apply(self.protocol_fee(policies)?.amount);
        // normalize
        Ok((protocol_fee.0 / eth::U256::exp10(18)).into())
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
}

fn apply_factor(amount: eth::U256, factor: f64) -> Option<eth::U256> {
    Some(amount.checked_mul(eth::U256::from_f64_lossy(factor * 10000000000.))? / 10000000000u128)
}

#[derive(Debug)]
pub struct Prices {
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

pub fn trade_surplus(
    kind: order::Side,
    executed: order::TargetAmount,
    sell: eth::Asset,
    buy: eth::Asset,
    prices: &ClearingPrices,
) -> Option<eth::Asset> {
    match kind {
        order::Side::Buy => {
            // scale limit sell to support partially fillable orders
            let limit_sell = sell
                .amount
                .0
                .checked_mul(executed.0)?
                .checked_div(buy.amount.0)?;
            // difference between limit sell and executed amount converted to sell token
            limit_sell.checked_sub(
                executed
                    .0
                    .checked_mul(prices.buy)?
                    .checked_div(prices.sell)?,
            )
        }
        order::Side::Sell => {
            // scale limit buy to support partially fillable orders
            let limit_buy = executed
                .0
                .checked_mul(buy.amount.0)?
                .checked_div(sell.amount.0)?;
            // difference between executed amount converted to buy token and limit buy
            executed
                .0
                .checked_mul(prices.sell)?
                .checked_div(prices.buy)?
                .checked_sub(limit_buy)
        }
    }
    .map(|surplus| match kind {
        order::Side::Buy => eth::Asset {
            amount: surplus.into(),
            token: sell.token,
        },
        order::Side::Sell => eth::Asset {
            amount: surplus.into(),
            token: buy.token,
        },
    })
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("multiple fee policies are not supported yet")]
    MultipleFeePolicies,
    #[error("fee policy not implemented yet")]
    UnimplementedFeePolicy,
    #[error("failed to calculate surplus for trade sell {0:?} buy {1:?}")]
    Surplus(eth::Asset, eth::Asset),
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error("factor {1} multiplication with {0} failed")]
    Factor(eth::U256, f64),
    #[error(transparent)]
    Math(#[from] MathError),
}

#[derive(Debug, thiserror::Error)]
pub enum MathError {
    #[error("overflow")]
    Overflow,
    #[error("division by zero")]
    DivisionByZero,
}
