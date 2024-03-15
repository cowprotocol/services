use {
    super::{
        error::Math,
        order::{self, Side},
    },
    crate::domain::{
        competition::{
            auction,
            solution::{fee, fee::adjust_quote_to_order_limits},
        },
        eth::{self, TokenAmount},
    },
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
            .try_fold(eth::Ether(0.into()), |acc, score| {
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
    fn surplus_over_limit_price(
        &self,
        limit_sell_amount: eth::U256,
        limit_buy_amount: eth::U256,
    ) -> Option<eth::Asset> {
        match self.side {
            Side::Buy => {
                // scale limit sell to support partially fillable orders
                let limit_sell = limit_sell_amount
                    .checked_mul(self.executed.into())?
                    .checked_div(limit_buy_amount)?;
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
                    .checked_mul(limit_buy_amount)?
                    .checked_div(limit_sell_amount)?;
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

    fn adjusted_order_limits(&self) -> Result<(eth::U256, eth::U256), Error> {
        match self.policies.first() {
            Some(order::FeePolicy::PriceImprovement {
                factor: _,
                max_volume_factor: _,
                quote,
            }) => adjust_quote_to_order_limits(
                fee::Order {
                    sell_amount: self.sell.amount.0,
                    buy_amount: self.buy.amount.0,
                    side: self.side,
                },
                fee::Quote {
                    sell_amount: quote.sell.amount.0,
                    buy_amount: quote.buy.amount.0,
                    fee_amount: quote.fee.amount.0,
                },
            )
            .map_err(|e| e.into()),
            _ => Ok((self.sell.amount.0, self.buy.amount.0)),
        }
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    ///
    /// Denominated in NATIVE token
    fn native_surplus(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let (limit_sell_amount, limit_buy_amount) = self.adjusted_order_limits()?;
        let surplus = self
            .surplus_over_limit_price(limit_sell_amount, limit_buy_amount)
            .ok_or(Error::Surplus(self.sell, self.buy))?;
        let price = prices
            .get(&surplus.token)
            .ok_or(Error::MissingPrice(surplus.token))?;

        Ok(price.in_eth(surplus.amount))
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// Denominated in SURPLUS token
    fn protocol_fee(&self) -> Result<eth::Asset, Error> {
        // TODO: support multiple fee policies
        if self.policies.len() > 1 {
            return Err(Error::MultipleFeePolicies);
        }

        let protocol_fee = |policy: &order::FeePolicy| match policy {
            order::FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => Ok(std::cmp::min(
                self.fee_from_surplus(factor)?,
                self.fee_from_volume(max_volume_factor)?,
            )),
            order::FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote: _,
            } => Ok(std::cmp::min(
                self.fee_from_surplus(factor)?,
                self.fee_from_volume(max_volume_factor)?,
            )),
            order::FeePolicy::Volume { factor: _ } => Err(Error::UnimplementedFeePolicy),
        };

        let protocol_fee = self.policies.first().map(protocol_fee).transpose();
        Ok(eth::Asset {
            token: self.surplus_token(),
            amount: protocol_fee?.unwrap_or(0.into()),
        })
    }

    fn fee_from_volume(&self, max_volume_factor: &f64) -> Result<TokenAmount, Error> {
        // Convert the executed amount to surplus token so it can be compared
        // with the surplus
        let executed_in_surplus_token: eth::TokenAmount = match self.side {
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
        }
        .into();
        let factor = match self.side {
            Side::Sell => max_volume_factor / (1.0 - max_volume_factor),
            Side::Buy => max_volume_factor / (1.0 + max_volume_factor),
        };
        executed_in_surplus_token
            .apply_factor(factor)
            .ok_or(Error::Factor(executed_in_surplus_token, factor))
    }

    fn fee_from_surplus(&self, factor: &f64) -> Result<TokenAmount, Error> {
        // If the surplus after all fees is X, then the original surplus before
        // protocol fee is X / (1 - factor)
        let (limit_sell_amount, limit_buy_amount) = self.adjusted_order_limits()?;
        let surplus = self
            .surplus_over_limit_price(limit_sell_amount, limit_buy_amount)
            .ok_or(Error::Surplus(self.sell, self.buy))?
            .amount;
        surplus
            .apply_factor(factor / (1.0 - factor))
            .ok_or(Error::Factor(surplus, *factor))
    }

    /// Protocol fee is defined by fee policies attached to the order.
    ///
    /// Denominated in NATIVE token
    fn native_protocol_fee(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let protocol_fee = self.protocol_fee()?;
        let price = prices
            .get(&protocol_fee.token)
            .ok_or(Error::MissingPrice(protocol_fee.token))?;

        Ok(price.in_eth(protocol_fee.amount))
    }

    fn surplus_token(&self) -> eth::TokenAddress {
        match self.side {
            Side::Buy => self.sell.token,
            Side::Sell => self.buy.token,
        }
    }
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
    Factor(eth::TokenAmount, f64),
    #[error(transparent)]
    Math(#[from] Math),
}
