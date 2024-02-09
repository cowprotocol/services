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
    super::trade::{Fee, Fulfillment, InvalidExecutedAmount},
    crate::domain::{
        competition::{
            order,
            order::{FeePolicy, Side},
        },
        eth,
    },
};

impl Fulfillment {
    /// Applies the protocol fee to the existing fulfillment creating a new one.
    pub fn with_protocol_fee(&self, prices: ClearingPrices) -> Result<Self, Error> {
        let protocol_fee = self.protocol_fee(prices)?;

        // Increase the fee by the protocol fee
        let fee = match self.surplus_fee() {
            None => {
                if !protocol_fee.is_zero() {
                    return Err(Error::ProtocolFeeOnStaticOrder);
                }
                Fee::Static
            }
            Some(fee) => {
                Fee::Dynamic((fee.0.checked_add(protocol_fee).ok_or(Error::Overflow)?).into())
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
                    .checked_sub(protocol_fee)
                    .ok_or(Error::Overflow)?,
            ),
        };

        Fulfillment::new(order, executed, fee).map_err(Into::into)
    }

    fn protocol_fee(&self, prices: ClearingPrices) -> Result<eth::U256, Error> {
        // TODO: support multiple fee policies
        if self.order().protocol_fees.len() > 1 {
            return Err(Error::MultipleFeePolicies);
        }

        match self.order().protocol_fees.first() {
            Some(FeePolicy::Surplus {
                factor,
                max_volume_factor,
            }) => self.calculate_fee(
                self.order().sell.amount.0,
                self.order().buy.amount.0,
                prices,
                *factor,
                *max_volume_factor,
            ),
            Some(FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            }) => {
                let (sell_amount, buy_amount) = adjusted_price_improvement_amounts(
                    self.order().sell.amount.0,
                    self.order().buy.amount.0,
                    self.order().side,
                    quote.sell.amount.0,
                    quote.buy.amount.0,
                    quote.fee.amount.0,
                )?;
                self.calculate_fee(sell_amount, buy_amount, prices, *factor, *max_volume_factor)
            }
            Some(FeePolicy::Volume { factor }) => self.fee_from_volume(prices, *factor),
            None => Ok(0.into()),
        }
    }

    /// Computes protocol fee compared to the given reference amounts taken from
    /// the order or a quote.
    fn calculate_fee(
        &self,
        reference_sell_amount: eth::U256,
        reference_buy_amount: eth::U256,
        prices: ClearingPrices,
        factor: f64,
        max_volume_factor: f64,
    ) -> Result<eth::U256, Error> {
        let fee_from_surplus =
            self.fee_from_surplus(reference_sell_amount, reference_buy_amount, prices, factor)?;
        let fee_from_volume = self.fee_from_volume(prices, max_volume_factor)?;
        // take the smaller of the two
        let protocol_fee = std::cmp::min(fee_from_surplus, fee_from_volume);
        tracing::debug!(uid=?self.order().uid, ?fee_from_surplus, ?fee_from_volume, ?protocol_fee, executed=?self.executed(), surplus_fee=?self.surplus_fee(), "calculated protocol fee");
        Ok(protocol_fee)
    }

    fn fee_from_surplus(
        &self,
        sell_amount: eth::U256,
        buy_amount: eth::U256,
        prices: ClearingPrices,
        factor: f64,
    ) -> Result<eth::U256, Error> {
        let executed = self.executed().0;
        let executed_sell_amount = match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                executed
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?
            }
            Side::Sell => executed,
        };
        // Sell slightly more `sell_token` to capture the `surplus_fee`
        let executed_sell_amount_with_fee = executed_sell_amount
            .checked_add(
                // surplus_fee is always expressed in sell token
                self.surplus_fee()
                    .map(|fee| fee.0)
                    .ok_or(Error::ProtocolFeeOnStaticOrder)?,
            )
            .ok_or(Error::Overflow)?;
        let surplus_in_sell_token = match self.order().side {
            Side::Buy => {
                // Scale to support partially fillable orders
                let limit_sell_amount = sell_amount
                    .checked_mul(executed)
                    .ok_or(Error::Overflow)?
                    .checked_div(buy_amount)
                    .ok_or(Error::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                limit_sell_amount
                    .checked_sub(executed_sell_amount_with_fee)
                    .unwrap_or(eth::U256::zero())
            }
            Side::Sell => {
                // Scale to support partially fillable orders
                let limit_buy_amount = buy_amount
                    .checked_mul(executed_sell_amount_with_fee)
                    .ok_or(Error::Overflow)?
                    .checked_div(sell_amount)
                    .ok_or(Error::DivisionByZero)?;
                // How much `buy_token` we get for `executed` amount of `sell_token`
                let executed_buy_amount = executed
                    .checked_mul(prices.sell)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.buy)
                    .ok_or(Error::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                let surplus = executed_buy_amount
                    .checked_sub(limit_buy_amount)
                    .unwrap_or(eth::U256::zero());
                // surplus in sell token
                surplus
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?
            }
        };
        apply_factor(surplus_in_sell_token, factor)
    }

    fn fee_from_volume(&self, prices: ClearingPrices, factor: f64) -> Result<eth::U256, Error> {
        let executed = self.executed().0;
        let executed_sell_amount = match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                executed
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?
            }
            Side::Sell => executed,
        };
        // Sell slightly more `sell_token` to capture the `surplus_fee`
        let executed_sell_amount_with_fee = executed_sell_amount
            .checked_add(
                // surplus_fee is always expressed in sell token
                self.surplus_fee()
                    .map(|fee| fee.0)
                    .ok_or(Error::ProtocolFeeOnStaticOrder)?,
            )
            .ok_or(Error::Overflow)?;
        apply_factor(executed_sell_amount_with_fee, factor)
    }
}

fn apply_factor(amount: eth::U256, factor: f64) -> Result<eth::U256, Error> {
    Ok(amount
        .checked_mul(eth::U256::from_f64_lossy(factor * 10000.))
        .ok_or(Error::Overflow)?
        / 10000)
}

fn adjusted_price_improvement_amounts(
    order_sell_amount: eth::U256,
    order_buy_amount: eth::U256,
    order_side: Side,
    quote_sell_amount: eth::U256,
    quote_buy_amount: eth::U256,
    quote_fee_amount: eth::U256,
) -> Result<(eth::U256, eth::U256), Error> {
    let quote_sell_amount = quote_sell_amount
        .checked_add(quote_fee_amount)
        .ok_or(Error::Overflow)?;

    match order_side {
        Side::Sell => {
            let scaled_buy_amount = quote_buy_amount
                .checked_mul(order_sell_amount)
                .ok_or(Error::Overflow)?
                .checked_div(quote_sell_amount)
                .ok_or(Error::DivisionByZero)?;
            let buy_amount = order_buy_amount.max(scaled_buy_amount);
            Ok((order_sell_amount, buy_amount))
        }
        Side::Buy => {
            let scaled_sell_amount = quote_sell_amount
                .checked_mul(order_buy_amount)
                .ok_or(Error::Overflow)?
                .checked_div(quote_buy_amount)
                .ok_or(Error::DivisionByZero)?;
            let sell_amount = order_sell_amount.min(scaled_sell_amount);
            Ok((sell_amount, order_buy_amount))
        }
    }
}

/// Uniform clearing prices at which the trade was executed.
#[derive(Debug, Clone, Copy)]
pub struct ClearingPrices {
    pub sell: eth::U256,
    pub buy: eth::U256,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("orders with non solver determined gas cost fees are not supported")]
    ProtocolFeeOnStaticOrder,
    #[error("multiple fee policies are not supported yet")]
    MultipleFeePolicies,
    #[error("overflow error while calculating protocol fee")]
    Overflow,
    #[error("division by zero error while calculating protocol fee")]
    DivisionByZero,
    #[error(transparent)]
    InvalidExecutedAmount(#[from] InvalidExecutedAmount),
}

// todo: should be removed once integration tests are implemented
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adjusted_price_improvement_amounts_for_sell_order() {
        let order_sell_amount = to_wei(20);
        let order_buy_amount = to_wei(19);
        let quote_sell_amount = to_wei(21);
        let quote_buy_amount = to_wei(18);
        let quote_fee_amount = to_wei(1);

        let (sell_amount, _) = adjusted_price_improvement_amounts(
            order_sell_amount,
            order_buy_amount,
            Side::Sell,
            quote_sell_amount,
            quote_buy_amount,
            quote_fee_amount,
        )
        .unwrap();

        assert_eq!(
            sell_amount, order_sell_amount,
            "Sell amount should match order sell amount for sell orders."
        );
    }

    #[test]
    fn test_adjusted_price_improvement_amounts_for_buy_order() {
        let order_sell_amount = to_wei(20);
        let order_buy_amount = to_wei(19);
        let quote_sell_amount = to_wei(21);
        let quote_buy_amount = to_wei(18);
        let quote_fee_amount = to_wei(1);

        let (_, buy_amount) = adjusted_price_improvement_amounts(
            order_sell_amount,
            order_buy_amount,
            Side::Buy,
            quote_sell_amount,
            quote_buy_amount,
            quote_fee_amount,
        )
        .unwrap();

        assert_eq!(
            buy_amount, order_buy_amount,
            "Buy amount should match order buy amount for buy orders."
        );
    }

    #[test]
    fn test_sell_order_with_quote_in_market_price() {
        let order_sell_amount = to_wei(10);
        let order_buy_amount = to_wei(20);
        let quote_sell_amount = to_wei(9);
        let quote_buy_amount = to_wei(25);
        let quote_fee_amount = to_wei(1);

        let (sell_amount, buy_amount) = adjusted_price_improvement_amounts(
            order_sell_amount,
            order_buy_amount,
            Side::Sell,
            quote_sell_amount,
            quote_buy_amount,
            quote_fee_amount,
        )
        .unwrap();

        assert_eq!(
            sell_amount, order_sell_amount,
            "Sell amount should be taken from the quote for sell orders in market price."
        );
        assert_eq!(
            buy_amount, quote_buy_amount,
            "Buy amount should reflect the improved market condition from the quote."
        );
    }

    #[test]
    fn test_buy_order_with_quote_in_market_price() {
        let order_sell_amount = to_wei(20);
        let order_buy_amount = to_wei(10);
        let quote_sell_amount = to_wei(17);
        let quote_buy_amount = to_wei(10);
        let quote_fee_amount = to_wei(1);

        let (sell_amount, buy_amount) = adjusted_price_improvement_amounts(
            order_sell_amount,
            order_buy_amount,
            Side::Buy,
            quote_sell_amount,
            quote_buy_amount,
            quote_fee_amount,
        )
        .unwrap();

        assert_eq!(
            sell_amount,
            quote_sell_amount + quote_fee_amount,
            "Sell amount should reflect the improved market condition from the quote for buy \
             orders."
        );
        assert_eq!(
            buy_amount, order_buy_amount,
            "Buy amount should be taken from the quote for buy orders in market price."
        );
    }

    pub fn to_wei(base: u32) -> eth::U256 {
        eth::U256::from(base) * eth::U256::exp10(18)
    }
}
