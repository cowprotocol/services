use {
    super::{
        error::Math,
        order::{self, Side},
    },
    crate::{
        domain::{competition::auction, eth},
        util::conv::u256::U256Ext,
    },
    bigdecimal::FromPrimitive,
    num::CheckedMul,
    number::conversions::big_rational_to_u256,
};

/// Scoring contains trades with values as they are expected by the settlement
/// contracts. This means that executed amounts and custom clearing prices have
/// the same values here and after being mined onchain. This allows us to use
/// the same math for calculating surplus and fees in the driver and in the
/// autopilot.
#[derive(Debug, Clone)]
pub struct Scoring {
    trades: Vec<Trade>,
}

impl Scoring {
    pub fn new(trades: Vec<Trade>) -> Self {
        Self { trades }
    }

    /// Score of a settlement as per CIP38
    ///
    /// Score of a settlement is a sum of scores of all user trades in the
    /// settlement. Score is defined as an order's surplus plus its protocol
    /// fee.
    ///
    /// Settlement score is valid only if all trade scores are valid.
    ///
    /// Denominated in NATIVE token
    pub fn score(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        self.trades
            .iter()
            .map(|trade| trade.score(prices))
            .try_fold(eth::Ether(eth::U256::zero()), |acc, score| {
                score.map(|score| acc + score)
            })
    }
}

// Trade represents a single trade in a settlement.
//
// It contains values as expected by the settlement contract. That means that
// clearing prices are adjusted to account for all fees (gas cost and protocol
// fees). Also, executed amount contains the fees for sell order.
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
    fn score(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
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
    fn native_surplus(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let surplus = self.surplus_token_price(prices)?.apply(
            self.surplus()
                .ok_or(Error::Surplus(self.sell, self.buy))?
                .amount,
        );
        // normalize
        Ok((surplus.0 / *UNIT).into())
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
                            Side::Sell => self
                                .executed
                                .0
                                .checked_mul(self.custom_price.sell)
                                .ok_or(Math::Overflow)?
                                .checked_div(self.custom_price.buy)
                                .ok_or(Math::DivisionByZero)?,
                            Side::Buy => self
                                .executed
                                .0
                                .checked_mul(self.custom_price.buy)
                                .ok_or(Math::Overflow)?
                                .checked_div(self.custom_price.sell)
                                .ok_or(Math::DivisionByZero)?,
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
                } => Err(Error::UnimplementedFeePolicy),
                order::FeePolicy::Volume { factor: _ } => Err(Error::UnimplementedFeePolicy),
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
    fn native_protocol_fee(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let protocol_fee = self
            .surplus_token_price(prices)?
            .apply(self.protocol_fee()?.amount);
        // normalize
        Ok((protocol_fee.0 / *UNIT).into())
    }

    fn surplus_token(&self) -> eth::TokenAddress {
        match self.side {
            Side::Buy => self.sell.token,
            Side::Sell => self.buy.token,
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
    let amount = amount.to_big_rational();
    let factor = num::BigRational::from_f64(factor)?;
    big_rational_to_u256(&amount.checked_mul(&factor)?).ok()
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
    #[error("fee policy not implemented yet")]
    UnimplementedFeePolicy,
    #[error("failed to calculate surplus for trade sell {0:?} buy {1:?}")]
    Surplus(eth::Asset, eth::Asset),
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error("factor {1} multiplication with {0} failed")]
    Factor(eth::U256, f64),
    #[error(transparent)]
    Math(#[from] Math),
}

lazy_static::lazy_static! {
    static ref UNIT: eth::U256 = eth::U256::from(1_000_000_000_000_000_000_u128);
}
