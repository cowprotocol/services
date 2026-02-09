pub use error::Error;
use {
    super::ExecutedProtocolFee,
    crate::domain::{
        self,
        OrderUid,
        auction::{self, order},
        eth,
        fee,
        settlement::transaction::{ClearingPrices, Prices},
    },
    error::Math,
    num::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub},
    number::u256_ext::U256Ext,
    std::collections::HashMap,
};

/// A trade containing bare minimum of onchain information required to calculate
/// the surplus, fees and score.
#[derive(Debug, Clone)]
pub struct Trade {
    pub uid: domain::OrderUid,
    pub sell: eth::Asset,
    pub buy: eth::Asset,
    pub side: order::Side,
    pub executed: order::TargetAmount,
    pub prices: Prices,
}

impl Trade {
    /// A general surplus function.
    ///
    /// Can return different types of surplus based on the input parameters.
    fn surplus_over(
        &self,
        prices: &ClearingPrices,
        price_limits: PriceLimits,
    ) -> Result<eth::SurplusTokenAmount, error::Math> {
        match self.side {
            order::Side::Buy => {
                // scale limit sell to support partially fillable orders
                let limit_sell = price_limits
                    .sell
                    .0
                    .checked_mul(self.executed.into())
                    .ok_or(error::Math::Overflow)?
                    .checked_div(price_limits.buy.0)
                    .ok_or(error::Math::DivisionByZero)?;
                let sold = self
                    .executed
                    .0
                    .checked_mul(prices.buy)
                    .ok_or(error::Math::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(error::Math::DivisionByZero)?;
                limit_sell.checked_sub(sold).ok_or(error::Math::Negative)
            }
            order::Side::Sell => {
                // scale limit buy to support partially fillable orders

                // `checked_ceil_div`` to be consistent with how settlement contract calculates
                // traded buy amounts
                // smallest allowed executed_buy_amount per settlement contract is
                // executed_sell_amount * ceil(price_limits.buy / price_limits.sell)
                let limit_buy = self
                    .executed
                    .0
                    .checked_mul(price_limits.buy.0)
                    .ok_or(error::Math::Overflow)?
                    .checked_ceil_div(&price_limits.sell.0)
                    .ok_or(error::Math::DivisionByZero)?;
                let bought = self
                    .executed
                    .0
                    .checked_mul(prices.sell)
                    .ok_or(error::Math::Overflow)?
                    .checked_ceil_div(&prices.buy)
                    .ok_or(error::Math::DivisionByZero)?;
                bought.checked_sub(limit_buy).ok_or(error::Math::Negative)
            }
        }
        .map(eth::SurplusTokenAmount)
    }

    /// Surplus based on custom clearing prices returns the surplus after all
    /// fees have been applied.
    pub fn surplus_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let surplus_amount = self.surplus_over_limit_price()?;
        let surplus_token = self.surplus_token();
        let price = prices
            .get(&surplus_token)
            .ok_or(Error::MissingPrice(surplus_token))?;

        Ok(price.in_eth(eth::TokenAmount(surplus_amount.0)))
    }

    /// Total fee (protocol fee + network fee). Equal to a surplus difference
    /// before and after applying the fees.
    pub fn fee_in_ether(&self, prices: &auction::Prices) -> Result<eth::Ether, Error> {
        let fee = self.fee()?;
        let fee_token = self.surplus_token();
        let price = prices
            .get(&fee_token)
            .ok_or(Error::MissingPrice(fee_token))?;
        Ok(price.in_eth(eth::TokenAmount(fee.0)))
    }

    /// Converts given surplus fee into sell token fee.
    fn fee_into_sell_token(
        &self,
        fee: eth::SurplusTokenAmount,
    ) -> Result<eth::SellTokenAmount, Error> {
        let fee_in_sell_token = match self.side {
            order::Side::Buy => fee.0,
            order::Side::Sell => fee
                .0
                .checked_mul(self.prices.uniform.buy)
                .ok_or(error::Math::Overflow)?
                .checked_div(self.prices.uniform.sell)
                .ok_or(error::Math::DivisionByZero)?,
        };
        Ok(eth::SellTokenAmount(fee_in_sell_token))
    }

    /// Total fee (protocol fee + network fee). Equal to a surplus difference
    /// before and after applying the fees.
    pub fn fee_in_sell_token(&self) -> Result<eth::SellTokenAmount, Error> {
        let fee = self.fee()?;
        self.fee_into_sell_token(fee)
    }

    /// Total fee (protocol fee + network fee). Equal to a surplus difference
    /// before and after applying the fees.
    fn fee(&self) -> Result<eth::SurplusTokenAmount, Error> {
        let fee = self
            .surplus_over_limit_price_before_fee()?
            .0
            .checked_sub(self.surplus_over_limit_price()?.0)
            .ok_or(error::Math::Negative)?;
        Ok(eth::SurplusTokenAmount(fee))
    }

    /// Protocol fees are defined by fee policies attached to the order.
    ///
    /// Denominated in SURPLUS token
    pub fn protocol_fees(
        &self,
        fee_policies: &HashMap<OrderUid, impl AsRef<[fee::Policy]>>,
    ) -> Result<Vec<ExecutedProtocolFee>, Error> {
        let policies = fee_policies
            .get(&self.uid)
            .map(|value| value.as_ref())
            .unwrap_or_default();
        let mut current_trade = self.clone();
        let mut total = eth::SurplusTokenAmount::default();
        let mut fees = vec![];
        for (i, policy) in policies.iter().enumerate().rev() {
            let fee = current_trade.protocol_fee(policy)?;
            fees.push(ExecutedProtocolFee {
                policy: *policy,
                fee: eth::Asset {
                    token: self.surplus_token(),
                    amount: eth::TokenAmount(fee.0),
                },
            });
            total = eth::SurplusTokenAmount(
                total
                    .0
                    .checked_add(fee.0)
                    .ok_or(Error::Math(Math::Overflow))?,
            );
            // Do not need to calculate the last custom prices because in the last iteration
            // the prices are not used anymore to calculate the protocol fee and an error
            // in this calculation fails the whole function unnecessarily.
            if i != 0 {
                current_trade.prices.custom = self.calculate_custom_prices(total)?;
            }
        }
        // Reverse the fees to have them in the same order as the policies
        fees.reverse();
        Ok(fees)
    }

    /// The effective amount that left the user's wallet including all fees.
    ///
    /// Note how the `executed` amount is used to build actual traded amounts.
    fn sell_amount(&self) -> Result<eth::TokenAmount, error::Math> {
        Ok(match self.side {
            order::Side::Sell => self.executed.0,
            order::Side::Buy => self
                .executed
                .0
                .checked_mul(self.prices.custom.buy)
                .ok_or(error::Math::Overflow)?
                .checked_div(self.prices.custom.sell)
                .ok_or(error::Math::DivisionByZero)?,
        }
        .into())
    }

    /// The effective amount the user received after all fees.
    ///
    /// Note how the `executed` amount is used to build actual traded amounts.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    fn buy_amount(&self) -> Result<eth::TokenAmount, error::Math> {
        Ok(match self.side {
            order::Side::Sell => self
                .executed
                .0
                .checked_mul(self.prices.custom.sell)
                .ok_or(error::Math::Overflow)?
                .checked_ceil_div(&self.prices.custom.buy)
                .ok_or(error::Math::DivisionByZero)?,
            order::Side::Buy => self.executed.0,
        }
        .into())
    }

    /// Derive new custom prices (given the current custom prices) to exclude
    /// the protocol fee from the trade.
    ///
    /// Note how the custom prices are expressed over actual traded amounts.
    fn calculate_custom_prices(
        &self,
        protocol_fee: eth::SurplusTokenAmount,
    ) -> Result<ClearingPrices, error::Math> {
        Ok(ClearingPrices {
            sell: match self.side {
                order::Side::Sell => self
                    .buy_amount()?
                    .0
                    .checked_add(protocol_fee.0)
                    .ok_or(error::Math::Overflow)?,
                order::Side::Buy => self.buy_amount()?.0,
            },
            buy: match self.side {
                order::Side::Sell => self.sell_amount()?.0,
                order::Side::Buy => self
                    .sell_amount()?
                    .0
                    .checked_sub(protocol_fee.0)
                    .ok_or(error::Math::Negative)?,
            },
        })
    }

    /// Protocol fee is defined by a fee policy attached to the order.
    fn protocol_fee(&self, fee_policy: &fee::Policy) -> Result<eth::SurplusTokenAmount, Error> {
        let fee = match fee_policy {
            fee::Policy::Surplus {
                factor,
                max_volume_factor,
            } => {
                let surplus = self.surplus_over_limit_price()?;
                std::cmp::min(
                    self.surplus_fee(surplus, (*factor).get())?,
                    self.volume_fee((*max_volume_factor).get())?,
                )
            }
            fee::Policy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => {
                let price_improvement = self.price_improvement(quote)?;
                std::cmp::min(
                    self.surplus_fee(price_improvement, (*factor).get())?,
                    self.volume_fee((*max_volume_factor).get())?,
                )
            }
            fee::Policy::Volume { factor } => self.volume_fee((*factor).get())?,
        };
        Ok(fee)
    }

    fn price_improvement(
        &self,
        quote: &domain::fee::Quote,
    ) -> Result<eth::SurplusTokenAmount, Error> {
        let surplus = self.surplus_over_quote(quote);
        // negative surplus is not error in this case, as solutions often have no
        // improvement over quote which results in negative surplus
        if let Err(error::Math::Negative) = surplus {
            return Ok(eth::SurplusTokenAmount(eth::U256::ZERO));
        }
        Ok(surplus?)
    }

    /// Uses custom prices to calculate the surplus after the protocol fee and
    /// network fee are applied.
    fn surplus_over_limit_price(&self) -> Result<eth::SurplusTokenAmount, error::Math> {
        let limit_price = PriceLimits {
            sell: self.sell.amount,
            buy: self.buy.amount,
        };
        self.surplus_over(&self.prices.custom, limit_price)
    }

    /// Uses uniform prices to calculate the surplus as if the protocol fee and
    /// network fee are not applied.
    fn surplus_over_limit_price_before_fee(&self) -> Result<eth::SurplusTokenAmount, error::Math> {
        let limit_price = PriceLimits {
            sell: self.sell.amount,
            buy: self.buy.amount,
        };
        self.surplus_over(&self.prices.uniform, limit_price)
    }

    fn surplus_over_quote(
        &self,
        quote: &domain::fee::Quote,
    ) -> Result<eth::SurplusTokenAmount, error::Math> {
        let quote = adjust_quote_to_order_limits(
            Order {
                sell: self.sell.amount,
                buy: self.buy.amount,
                side: self.side,
            },
            Quote {
                sell: quote.sell_amount.into(),
                buy: quote.buy_amount.into(),
                fee: quote.fee.into(),
            },
        )?;
        self.surplus_over(&self.prices.custom, quote)
    }

    /// Protocol fee as a cut of surplus.
    fn surplus_fee(
        &self,
        surplus: eth::SurplusTokenAmount,
        factor: f64,
    ) -> Result<eth::SurplusTokenAmount, Error> {
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
            .0
            .checked_mul_f64(factor / (1.0 - factor))
            .ok_or(error::Math::Overflow)?;

        Ok(eth::SurplusTokenAmount(fee))
    }

    /// Protocol fee as a cut of the trade volume.
    fn volume_fee(&self, factor: f64) -> Result<eth::SurplusTokenAmount, Error> {
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
        let executed_in_surplus_token = match self.side {
            order::Side::Buy => self.sell_amount()?,
            order::Side::Sell => self.buy_amount()?,
        };
        let factor = match self.side {
            order::Side::Sell => factor / (1.0 - factor),
            order::Side::Buy => factor / (1.0 + factor),
        };

        Ok(eth::SurplusTokenAmount(
            executed_in_surplus_token
                .0
                .checked_mul_f64(factor)
                .ok_or(error::Math::Overflow)?,
        ))
    }

    fn surplus_token(&self) -> eth::TokenAddress {
        match self.side {
            order::Side::Buy => self.sell.token,
            order::Side::Sell => self.buy.token,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PriceLimits {
    pub sell: eth::TokenAmount,
    pub buy: eth::TokenAmount,
}

/// This function adjusts quote amounts to directly compare them with the
/// order's limits, ensuring a meaningful comparison for potential price
/// improvements. It scales quote amounts when necessary, accounting for quote
/// fees, to align the quote's sell or buy amounts with the order's
/// corresponding amounts. This adjustment is crucial for assessing whether the
/// quote offers a price improvement over the order's conditions.
///
/// Scaling is needed because the quote and the order might not be directly
/// comparable due to differences in amounts and the inclusion of fees in the
/// quote. By adjusting the quote's amounts to match the order's sell or buy
/// amounts, we can accurately determine if the quote provides a better rate
/// than the order's limits.
///
/// ## Examples
/// For the specific examples, consider the following unit tests:
/// - test_adjust_quote_to_out_market_sell_order_limits
/// - test_adjust_quote_to_out_market_buy_order_limits
/// - test_adjust_quote_to_in_market_sell_order_limits
/// - test_adjust_quote_to_in_market_buy_order_limits
fn adjust_quote_to_order_limits(order: Order, quote: Quote) -> Result<PriceLimits, error::Math> {
    match order.side {
        order::Side::Sell => {
            let quote_buy_amount = quote
                .buy
                .checked_sub(
                    &quote
                        .fee
                        .checked_mul(&quote.buy)
                        .ok_or(error::Math::Overflow)?
                        .checked_div(&quote.sell)
                        .ok_or(error::Math::DivisionByZero)?,
                )
                .ok_or(error::Math::Negative)?;
            let scaled_buy_amount = quote_buy_amount
                .checked_mul(&order.sell)
                .ok_or(error::Math::Overflow)?
                .checked_div(&quote.sell)
                .ok_or(error::Math::DivisionByZero)?;
            let buy_amount = order.buy.max(scaled_buy_amount);
            Ok(PriceLimits {
                sell: order.sell,
                buy: buy_amount,
            })
        }
        order::Side::Buy => {
            let quote_sell_amount = quote
                .sell
                .checked_add(&quote.fee)
                .ok_or(error::Math::Overflow)?;
            let scaled_sell_amount = quote_sell_amount
                .checked_mul(&order.buy)
                .ok_or(error::Math::Overflow)?
                .checked_div(&quote.buy)
                .ok_or(error::Math::DivisionByZero)?;
            let sell_amount = order.sell.min(scaled_sell_amount);
            Ok(PriceLimits {
                sell: sell_amount,
                buy: order.buy,
            })
        }
    }
}

#[derive(Clone)]
struct Order {
    pub sell: eth::TokenAmount,
    pub buy: eth::TokenAmount,
    pub side: order::Side,
}

#[derive(Clone)]
struct Quote {
    pub sell: eth::TokenAmount,
    pub buy: eth::TokenAmount,
    pub fee: eth::TokenAmount,
}

impl From<&super::Fulfillment> for Trade {
    fn from(fulfillment: &super::Fulfillment) -> Self {
        Self {
            uid: fulfillment.uid,
            sell: fulfillment.sell,
            buy: fulfillment.buy,
            side: fulfillment.side,
            executed: fulfillment.executed,
            prices: fulfillment.prices,
        }
    }
}

impl From<&super::Jit> for Trade {
    fn from(jit: &super::Jit) -> Self {
        Self {
            uid: jit.uid,
            sell: jit.sell,
            buy: jit.buy,
            side: jit.side,
            executed: jit.executed,
            prices: jit.prices,
        }
    }
}

impl From<&super::Trade> for Trade {
    fn from(trade: &super::Trade) -> Self {
        match trade {
            super::Trade::Fulfillment(fulfillment) => fulfillment.into(),
            super::Trade::Jit(jit) => jit.into(),
        }
    }
}
pub mod error {
    use crate::domain::eth;

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        #[error("missing native price for token {0:?}")]
        MissingPrice(eth::TokenAddress),
        #[error(transparent)]
        Math(#[from] Math),
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Math {
        #[error("overflow")]
        Overflow,
        #[error("division by zero")]
        DivisionByZero,
        #[error("negative")]
        Negative,
    }
}
