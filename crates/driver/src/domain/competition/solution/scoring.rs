//! Scoring of a solution.
//!
//! Scoring is done on a solution that is identical to the one that will appear
//! onchain. This means that all fees are already applied to the trades and the
//! executed amounts are adjusted to account for all fees (gas cost and protocol
//! fees). No further changes are expected to be done on solution by the driver
//! after scoring.

use {
    super::{
        error::Math,
        order::{self, Side},
    },
    crate::{
        domain::{
            competition::{
                auction,
                order::fees::Quote,
                solution::{fee, fee::adjust_quote_to_order_limits},
                PriceLimits,
            },
            eth::{self},
        },
        util::conv::u256::U256Ext,
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
        self.trades.iter().map(|trade| trade.score(prices)).sum()
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
        tracing::debug!("Scoring trade {:?}", self);
        Ok(self.native_surplus(prices)? + self.native_protocol_fee(prices)?)
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied and calculated over the price limits.
    ///
    /// Denominated in SURPLUS token
    fn surplus_over(&self, price_limits: PriceLimits) -> Result<eth::Asset, Math> {
        match self.side {
            Side::Buy => {
                // scale limit sell to support partially fillable orders
                let limit_sell = price_limits
                    .sell
                    .0
                    .checked_mul(self.executed.into())
                    .ok_or(Math::Overflow)?
                    .checked_div(price_limits.buy.0)
                    .ok_or(Math::DivisionByZero)?;
                let sold = self
                    .executed
                    .0
                    .checked_mul(self.custom_price.buy)
                    .ok_or(Math::Overflow)?
                    .checked_div(self.custom_price.sell)
                    .ok_or(Math::DivisionByZero)?;
                limit_sell.checked_sub(sold).ok_or(Math::Negative)
            }
            Side::Sell => {
                // scale limit buy to support partially fillable orders
                let limit_buy = self
                    .executed
                    .0
                    .checked_mul(price_limits.buy.0)
                    .ok_or(Math::Overflow)?
                    .checked_div(price_limits.sell.0)
                    .ok_or(Math::DivisionByZero)?;
                let bought = self
                    .executed
                    .0
                    .checked_mul(self.custom_price.sell)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&self.custom_price.buy)
                    .ok_or(Math::DivisionByZero)?;
                bought.checked_sub(limit_buy).ok_or(Math::Negative)
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
        let surplus = self.surplus_over_limit_price()?;
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
            } => {
                let surplus = self.surplus_over_limit_price()?;
                let fee = std::cmp::min(
                    self.surplus_fee(surplus, *factor)?.amount,
                    self.volume_fee(*max_volume_factor)?.amount,
                );
                Ok::<eth::TokenAmount, Error>(fee)
            }
            order::FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => {
                let price_improvement = self.price_improvement(quote)?;
                let fee = std::cmp::min(
                    self.surplus_fee(price_improvement, *factor)?.amount,
                    self.volume_fee(*max_volume_factor)?.amount,
                );
                Ok(fee)
            }
            order::FeePolicy::Volume { factor } => Ok(self.volume_fee(*factor)?.amount),
        };

        let protocol_fee = self.policies.first().map(protocol_fee).transpose();
        Ok(eth::Asset {
            token: self.surplus_token(),
            amount: protocol_fee?.unwrap_or(0.into()),
        })
    }

    fn price_improvement(&self, quote: &Quote) -> Result<eth::Asset, Error> {
        let quote = adjust_quote_to_order_limits(
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
        )?;
        let surplus = self.surplus_over(quote);
        // negative surplus is not error in this case, as solutions often have no
        // improvement over quote which results in negative surplus
        if let Err(Math::Negative) = surplus {
            return Ok(eth::Asset {
                token: self.surplus_token(),
                amount: 0.into(),
            });
        }
        Ok(surplus?)
    }

    fn surplus_over_limit_price(&self) -> Result<eth::Asset, Error> {
        let limit_price = PriceLimits {
            sell: self.sell.amount,
            buy: self.buy.amount,
        };
        Ok(self.surplus_over(limit_price)?)
    }

    /// Protocol fee as a cut of surplus, denominated in SURPLUS token
    fn surplus_fee(&self, surplus: eth::Asset, factor: f64) -> Result<eth::Asset, Error> {
        // Surplus fee is specified as a `factor` from raw surplus (before fee). Since
        // this module works with trades that already have the protocol fee applied, we
        // need to calculate the protocol fee as an observation of the eventually traded
        // amounts using a different factor `factor'`.
        //
        // The protocol fee before being applied is:
        //    fee = surplus_before_fee * factor
        // The protocol fee after being applied is:
        //    fee = surplus_after_fee * factor'
        // Also:
        //    surplus_after_fee = surplus_before_fee - fee
        // So:
        //    factor' = fee / surplus_after_fee = fee / (surplus_before_fee -
        // fee) = fee / ((fee / factor) - fee) = factor / (1 - factor)
        //
        // Finally:
        //     fee = surplus_after_fee * factor / (1 - factor)
        let fee = surplus
            .amount
            .apply_factor(factor / (1.0 - factor))
            .ok_or(Math::Overflow)?;

        Ok(eth::Asset {
            token: surplus.token,
            amount: fee,
        })
    }

    /// Protocol fee as a cut of the trade volume, denominated in SURPLUS token
    fn volume_fee(&self, factor: f64) -> Result<eth::Asset, Error> {
        // Volume fee is specified as a `factor` from raw volume (before fee). Since
        // this module works with trades that already have the protocol fee applied, we
        // need to calculate the protocol fee as an observation of a the eventually
        // traded amount using a different factor `factor'` .
        //
        // The protocol fee before being applied is:
        // case Sell: fee = traded_buy_amount * factor, resulting in the REDUCED
        // buy amount
        // case Buy: fee = traded_sell_amount * factor, resulting in the INCREASED
        // sell amount
        //
        // The protocol fee after being applied is:
        // case Sell: fee = traded_buy_amount' * factor',
        // case Buy: fee = traded_sell_amount' * factor',
        //
        // Also:
        // case Sell: traded_buy_amount' = traded_buy_amount - fee
        // case Buy: traded_sell_amount' = traded_sell_amount + fee
        //
        // So:
        // case Sell: factor' = fee / (traded_buy_amount - fee) = fee / (fee /
        // factor - fee) = factor / (1 - factor)
        // case Buy: factor' = fee / (traded_sell_amount + fee) = fee / (fee /
        // factor + fee) = factor / (1 + factor)
        //
        // Finally:
        // case Sell: fee = traded_buy_amount' * factor / (1 - factor)
        // case Buy: fee = traded_sell_amount' * factor / (1 + factor)
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
            Side::Sell => factor / (1.0 - factor),
            Side::Buy => factor / (1.0 + factor),
        };

        Ok(eth::Asset {
            token: self.surplus_token(),
            amount: {
                executed_in_surplus_token
                    .apply_factor(factor)
                    .ok_or(Math::Overflow)?
            },
        })
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
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
}
