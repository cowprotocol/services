//! Applies the protocol fee to the solution received from the solver.
//!
//! Solvers respond differently for the sell and buy orders.
//!
//! EXAMPLES:
//!
//! SELL ORDER
//! Selling 1 WETH for at least `x` amount of USDC. Solvers respond with
//! Fee = 0.05 WETH (always expressed in sell token)
//! Executed = 0.95 WETH (always expressed in target token)
//!
//! This response is adjusted by the protocol fee of 0.1 WETH:
//! Fee = 0.05 WETH + 0.1 WETH = 0.15 WETH
//! Executed = 0.95 WETH - 0.1 WETH = 0.85 WETH
//!
//! BUY ORDER
//! Buying 1 WETH for at most `x` amount of USDC. Solvers respond with
//! Fee = 10 USDC (always expressed in sell token)
//! Executed = 1 WETH (always expressed in target token)
//!
//! This response is adjusted by the protocol fee of 5 USDC:
//! Fee = 10 USDC + 5 USDC = 15 USDC
//! Executed = 1 WETH

use {
    super::{
        error::Math,
        trade::{self, ClearingPrices, Fee, Fulfillment},
    },
    crate::domain::{
        competition::{
            order,
            order::{FeePolicy, Side},
            PriceLimits,
        },
        eth,
    },
    bigdecimal::Zero,
};

impl Fulfillment {
    /// Applies the protocol fee to the existing fulfillment creating a new one.
    pub fn with_protocol_fee(&self, prices: ClearingPrices) -> Result<Self, Error> {
        let protocol_fee = self.protocol_fee_in_sell_token(prices)?;

        // Increase the fee by the protocol fee
        let fee = match self.surplus_fee() {
            None => {
                if !protocol_fee.is_zero() {
                    return Err(Error::ProtocolFeeOnStaticOrder);
                }
                Fee::Static
            }
            Some(fee) => {
                Fee::Dynamic((fee.0.checked_add(protocol_fee.0).ok_or(Math::Overflow)?).into())
            }
        };

        // Reduce the executed amount by the protocol fee. This is because solvers are
        // unaware of the protocol fee that driver introduces and they only account
        // for their own fee.
        let order = self.order().clone();
        let executed = match order.side {
            order::Side::Buy => self.executed(),
            order::Side::Sell => order::TargetAmount(
                self.executed()
                    .0
                    .checked_sub(protocol_fee.0)
                    .ok_or(Math::Overflow)?,
            ),
        };

        Fulfillment::new(order, executed, fee).map_err(Into::into)
    }

    /// Computed protocol fee in surplus token.
    fn protocol_fee(&self, prices: ClearingPrices) -> Result<eth::TokenAmount, Error> {
        // TODO: support multiple fee policies
        if self.order().protocol_fees.len() > 1 {
            return Err(Error::MultipleFeePolicies);
        }

        match self.order().protocol_fees.first() {
            Some(FeePolicy::Surplus {
                factor,
                max_volume_factor,
            }) => self.calculate_fee(
                PriceLimits {
                    sell: self.order().sell.amount,
                    buy: self.order().buy.amount,
                },
                prices,
                *factor,
                *max_volume_factor,
            ),
            Some(FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            }) => {
                let price_limits = adjust_quote_to_order_limits(
                    Order {
                        sell_amount: self.order().sell.amount.0,
                        buy_amount: self.order().buy.amount.0,
                        side: self.order().side,
                    },
                    Quote {
                        sell_amount: quote.sell.amount.0,
                        buy_amount: quote.buy.amount.0,
                        fee_amount: quote.fee.amount.0,
                    },
                )?;
                self.calculate_fee(price_limits, prices, *factor, *max_volume_factor)
            }
            Some(FeePolicy::Volume { factor }) => self.fee_from_volume(prices, *factor),
            None => Ok(0.into()),
        }
    }

    /// Computes protocol fee compared to the given limit amounts taken from
    /// the order or a quote.
    ///
    /// The protocol fee is computed in surplus token.
    fn calculate_fee(
        &self,
        price_limits: PriceLimits,
        prices: ClearingPrices,
        factor: f64,
        max_volume_factor: f64,
    ) -> Result<eth::TokenAmount, Error> {
        let fee_from_surplus =
            self.fee_from_surplus(price_limits.sell.0, price_limits.buy.0, prices, factor)?;
        let fee_from_volume = self.fee_from_volume(prices, max_volume_factor)?;
        // take the smaller of the two
        let protocol_fee = std::cmp::min(fee_from_surplus, fee_from_volume);
        tracing::debug!(uid=?self.order().uid, ?fee_from_surplus, ?fee_from_volume, ?protocol_fee, executed=?self.executed(), surplus_fee=?self.surplus_fee(), "calculated protocol fee");
        Ok(protocol_fee)
    }

    /// Computes the surplus fee in the surplus token.
    fn fee_from_surplus(
        &self,
        sell_amount: eth::U256,
        buy_amount: eth::U256,
        prices: ClearingPrices,
        factor: f64,
    ) -> Result<eth::TokenAmount, Error> {
        let surplus = self.surplus_over_reference_price(sell_amount, buy_amount, prices)?;
        surplus
            .apply_factor(factor)
            .ok_or(Math::Overflow)
            .map_err(Into::into)
    }

    /// Computes the volume based fee in surplus token
    ///
    /// The volume is defined as a full sell amount (including fees) for buy
    /// order, or a full buy amount for sell order.
    fn fee_from_volume(
        &self,
        prices: ClearingPrices,
        factor: f64,
    ) -> Result<eth::TokenAmount, Error> {
        let volume = match self.order().side {
            Side::Buy => self.sell_amount(&prices)?,
            Side::Sell => self.buy_amount(&prices)?,
        };
        volume
            .apply_factor(factor)
            .ok_or(Math::Overflow)
            .map_err(Into::into)
    }

    /// Returns the protocol fee denominated in the sell token.
    fn protocol_fee_in_sell_token(
        &self,
        prices: ClearingPrices,
    ) -> Result<eth::TokenAmount, Error> {
        let fee_in_sell_token = match self.order().side {
            Side::Buy => self.protocol_fee(prices)?,
            Side::Sell => self
                .protocol_fee(prices)?
                .0
                .checked_mul(prices.buy)
                .ok_or(Math::Overflow)?
                .checked_div(prices.sell)
                .ok_or(Math::DivisionByZero)?
                .into(),
        };
        Ok(fee_in_sell_token)
    }
}

#[derive(Clone)]
pub struct Order {
    pub sell_amount: eth::U256,
    pub buy_amount: eth::U256,
    pub side: Side,
}

#[derive(Clone)]
pub struct Quote {
    pub sell_amount: eth::U256,
    pub buy_amount: eth::U256,
    pub fee_amount: eth::U256,
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
pub fn adjust_quote_to_order_limits(order: Order, quote: Quote) -> Result<PriceLimits, Math> {
    match order.side {
        Side::Sell => {
            let quote_buy_amount = quote
                .buy_amount
                .checked_sub(quote.fee_amount / quote.sell_amount * quote.buy_amount)
                .ok_or(Math::Negative)?;
            let scaled_buy_amount = quote_buy_amount
                .checked_mul(order.sell_amount)
                .ok_or(Math::Overflow)?
                .checked_div(quote.sell_amount)
                .ok_or(Math::DivisionByZero)?;
            let buy_amount = order.buy_amount.max(scaled_buy_amount);
            Ok(PriceLimits {
                sell: order.sell_amount.into(),
                buy: buy_amount.into(),
            })
        }
        Side::Buy => {
            let quote_sell_amount = quote
                .sell_amount
                .checked_add(quote.fee_amount)
                .ok_or(Math::Overflow)?;
            let scaled_sell_amount = quote_sell_amount
                .checked_mul(order.buy_amount)
                .ok_or(Math::Overflow)?
                .checked_div(quote.buy_amount)
                .ok_or(Math::DivisionByZero)?;
            let sell_amount = order.sell_amount.min(scaled_sell_amount);
            Ok(PriceLimits {
                sell: sell_amount.into(),
                buy: order.buy_amount.into(),
            })
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("multiple fee policies are not supported yet")]
    MultipleFeePolicies,
    #[error("orders with non solver determined gas cost fees are not supported")]
    ProtocolFeeOnStaticOrder,
    #[error(transparent)]
    Math(#[from] Math),
    #[error(transparent)]
    Fulfillment(#[from] trade::Error),
}

// todo: should be removed once integration tests are implemented
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjust_quote_to_out_market_sell_order_limits() {
        let order = Order {
            sell_amount: to_wei(20),
            buy_amount: to_wei(19),
            side: Side::Sell,
        };
        let quote = Quote {
            sell_amount: to_wei(21),
            buy_amount: to_wei(18),
            fee_amount: to_wei(1),
        };
        let limit = adjust_quote_to_order_limits(order.clone(), quote).unwrap();

        assert_eq!(
            limit.sell.0, order.sell_amount,
            "Sell amount should match order sell amount for sell orders."
        );
    }

    #[test]
    fn test_adjust_quote_to_out_market_buy_order_limits() {
        let order = Order {
            sell_amount: to_wei(20),
            buy_amount: to_wei(19),
            side: Side::Buy,
        };
        let quote = Quote {
            sell_amount: to_wei(21),
            buy_amount: to_wei(18),
            fee_amount: to_wei(1),
        };

        let limit = adjust_quote_to_order_limits(order.clone(), quote).unwrap();

        assert_eq!(
            limit.buy.0, order.buy_amount,
            "Buy amount should match order buy amount for buy orders."
        );
    }

    #[test]
    fn test_adjust_quote_to_in_market_sell_order_limits() {
        let order = Order {
            sell_amount: to_wei(10),
            buy_amount: to_wei(20),
            side: Side::Sell,
        };
        let quote = Quote {
            sell_amount: to_wei(9),
            buy_amount: to_wei(25),
            fee_amount: to_wei(1),
        };

        let limit = adjust_quote_to_order_limits(order.clone(), quote.clone()).unwrap();

        assert_eq!(
            limit.sell.0, order.sell_amount,
            "Sell amount should be taken from the order for sell orders in market price."
        );
        assert_eq!(
            limit.buy.0, quote.buy_amount,
            "Buy amount should reflect the improved market condition from the quote."
        );
    }

    #[test]
    fn test_adjust_quote_to_in_market_buy_order_limits() {
        let order = Order {
            sell_amount: to_wei(20),
            buy_amount: to_wei(10),
            side: Side::Buy,
        };
        let quote = Quote {
            sell_amount: to_wei(17),
            buy_amount: to_wei(10),
            fee_amount: to_wei(1),
        };

        let limit = adjust_quote_to_order_limits(order.clone(), quote.clone()).unwrap();

        assert_eq!(
            limit.sell.0,
            quote.sell_amount + quote.fee_amount,
            "Sell amount should reflect the improved market condition from the quote for buy \
             orders."
        );
        assert_eq!(
            limit.buy.0, order.buy_amount,
            "Buy amount should be taken from the order for buy orders in market price."
        );
    }

    pub fn to_wei(base: u32) -> eth::U256 {
        eth::U256::from(base) * eth::U256::exp10(18)
    }
}
