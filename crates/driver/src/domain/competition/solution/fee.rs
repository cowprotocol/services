//! Applies the protocol fee to the solution received from the solver.
//!
//! Solvers respond differently for the sell and buy orders.
//!
//! EXAMPLES:
//!
//! SELL ORDER
//! Selling 1 WETH for at least `x` amount of USDC. Solvers respond with
//! Fee = 0.05 WETH
//! Executed = 0.95 WETH
//!
//! This response is adjusted by the protocol fee of 0.1 WETH:
//! Fee = 0.05 WETH + 0.1 WETH = 0.15 WETH
//! Executed = 0.95 WETH - 0.1 WETH = 0.85 WETH
//!
//! BUY ORDER
//! Buying 1 WETH for at most `x` amount of USDC. Solvers respond with
//! Fee = 0.05 WETH
//! Executed = 1 WETH
//!
//! This response is adjusted by the protocol fee of 0.1 WETH:
//! Fee = 0.05 WETH + 0.1 WETH = 0.15 WETH
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
        let fee = match self.dynamic_fee() {
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
        if self.order().fee_policies.len() > 1 {
            return Err(Error::MultipleFeePolicies);
        }

        let mut protocol_fee = eth::U256::zero();
        if let Some(fee_policy) = self.order().fee_policies.first() {
            match fee_policy {
                FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                } => {
                    let price_improvement_fee = self.price_improvement_fee(prices, *factor)?;
                    println!("price_improvement_fee: {}", price_improvement_fee);
                    let max_volume_fee = self.volume_fee(prices, *max_volume_factor)?;
                    println!("max_volume_fee: {}", max_volume_fee);
                    // take the smaller of the two
                    protocol_fee = std::cmp::min(price_improvement_fee, max_volume_fee);
                }
                FeePolicy::Volume { factor } => {
                    protocol_fee = self.volume_fee(prices, *factor)?;
                }
            }
        }
        Ok(protocol_fee)
    }

    fn price_improvement_fee(
        &self,
        prices: ClearingPrices,
        factor: f64,
    ) -> Result<eth::U256, Error> {
        let sell_amount = self.order().sell.amount.0;
        let buy_amount = self.order().buy.amount.0;
        let executed = self.executed().0;
        let surplus_fee = self
            .dynamic_fee()
            .map(|fee| fee.0)
            .ok_or(Error::ProtocolFeeOnStaticOrder)?;
        match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                let executed_sell_amount = executed
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?;
                // Sell slightly more `sell_token` to capture the `surplus_fee`
                let executed_sell_amount_with_fee = executed_sell_amount
                    .checked_add(surplus_fee)
                    .ok_or(Error::Overflow)?;
                // Scale to support partially fillable orders
                let limit_sell_amount = sell_amount
                    .checked_mul(executed)
                    .ok_or(Error::Overflow)?
                    .checked_div(buy_amount)
                    .ok_or(Error::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                let surplus = limit_sell_amount
                    .checked_sub(executed_sell_amount_with_fee)
                    .unwrap_or(eth::U256::zero());
                Ok(surplus
                    .checked_mul(eth::U256::from_f64_lossy(factor * 100.))
                    .ok_or(Error::Overflow)?
                    / 100)
            }
            Side::Sell => {
                // How much `buy_token` we get for `executed` amount of `sell_token`
                let executed_buy_amount = executed
                    .checked_mul(prices.sell)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.buy)
                    .ok_or(Error::DivisionByZero)?;
                let executed_sell_amount_with_fee =
                    executed.checked_add(surplus_fee).ok_or(Error::Overflow)?;
                // Scale to support partially fillable orders
                let limit_buy_amount = buy_amount
                    .checked_mul(executed_sell_amount_with_fee)
                    .ok_or(Error::Overflow)?
                    .checked_div(sell_amount)
                    .ok_or(Error::DivisionByZero)?;
                // Remaining surplus after fees
                // Do not return error if `checked_sub` fails because violated limit prices will
                // be caught by simulation
                let surplus = executed_buy_amount
                    .checked_sub(limit_buy_amount)
                    .unwrap_or(eth::U256::zero());
                let surplus_in_sell_token = surplus
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?;
                Ok(surplus_in_sell_token
                    .checked_mul(eth::U256::from_f64_lossy(factor * 100.))
                    .ok_or(Error::Overflow)?
                    / 100)
            }
        }
    }

    fn volume_fee(&self, prices: ClearingPrices, factor: f64) -> Result<eth::U256, Error> {
        let executed = self.executed().0;
        let surplus_fee = self
            .dynamic_fee()
            .map(|fee| fee.0)
            .ok_or(Error::ProtocolFeeOnStaticOrder)?;
        match self.order().side {
            Side::Buy => {
                // How much `sell_token` we need to sell to buy `executed` amount of `buy_token`
                let executed_sell_amount = executed
                    .checked_mul(prices.buy)
                    .ok_or(Error::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Error::DivisionByZero)?;
                // Sell slightly more `sell_token` to capture the `surplus_fee`
                let executed_sell_amount_with_fee = executed_sell_amount
                    .checked_add(surplus_fee)
                    .ok_or(Error::Overflow)?;
                Ok(executed_sell_amount_with_fee
                    .checked_mul(eth::U256::from_f64_lossy(factor * 100.))
                    .ok_or(Error::Overflow)?
                    / 100)
            }
            Side::Sell => Ok(executed
                .checked_mul(eth::U256::from_f64_lossy(factor * 100.))
                .ok_or(Error::Overflow)?
                / 100),
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
