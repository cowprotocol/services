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
        trade::{ClearingPrices, Fee, Fulfillment},
    },
    crate::domain::competition::{
        PriceLimits,
        order::{self, FeePolicy, Side},
        solution::error::Trade,
    },
    bigdecimal::Zero,
    eth_domain_types as eth,
};

impl Fulfillment {
    /// Applies the protocol fees to the existing fulfillment creating a new
    /// one.
    pub fn with_protocol_fees(&self, prices: ClearingPrices) -> Result<Self, Error> {
        let mut current_fulfillment = self.clone();
        for protocol_fee in &self.order().protocol_fees {
            current_fulfillment = current_fulfillment.with_protocol_fee(prices, protocol_fee)?;
        }
        current_fulfillment.ensure_limit_price_respected(prices)?;
        Ok(current_fulfillment)
    }

    /// Verifies that the effective execution price still respects the order's
    /// limit price after all protocol fees have been applied.
    fn ensure_limit_price_respected(&self, prices: ClearingPrices) -> Result<(), Error> {
        let buy = self.buy_amount(&prices)?;
        let sell = self.sell_amount(&prices)?;
        let order = self.order();
        // buy_received / sell_paid >= buy_limit / sell_limit
        // Rewritten to avoid division: buy_received * sell_limit >= sell_paid *
        // buy_limit
        let left = buy
            .0
            .checked_mul(order.sell.amount.0)
            .ok_or(Math::Overflow)?;
        let right = sell
            .0
            .checked_mul(order.buy.amount.0)
            .ok_or(Math::Overflow)?;
        if left < right {
            return Err(Error::LimitPriceViolatedByProtocolFees);
        }
        Ok(())
    }

    /// Applies the protocol fee to the existing fulfillment creating a new one.
    fn with_protocol_fee(
        &self,
        prices: ClearingPrices,
        protocol_fee: &FeePolicy,
    ) -> Result<Self, Error> {
        let protocol_fee = self.protocol_fee_in_sell_token(prices, protocol_fee)?;

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

        Fulfillment::new(order, executed, fee, self.haircut_fee()).map_err(Into::into)
    }

    /// Computed protocol fee in surplus token.
    fn protocol_fee(
        &self,
        prices: ClearingPrices,
        protocol_fee: &FeePolicy,
    ) -> Result<eth::TokenAmount, Error> {
        match protocol_fee {
            FeePolicy::Surplus {
                factor,
                max_volume_factor,
            } => self.calculate_fee(
                PriceLimits {
                    sell: self.order().sell.amount,
                    buy: self.order().buy.amount,
                },
                prices,
                *factor,
                *max_volume_factor,
            ),
            FeePolicy::PriceImprovement {
                factor,
                max_volume_factor,
                quote,
            } => {
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
            FeePolicy::Volume { factor } => {
                let fee_from_volume = self.fee_from_volume(prices, *factor)?;
                tracing::debug!(
                    uid = ?self.order().uid,
                    ?fee_from_volume,
                    executed = ?self.executed(),
                    surplus_fee = ?self.surplus_fee(),
                    "calculated protocol fee"
                );
                Ok(fee_from_volume)
            }
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
        tracing::debug!(
            uid = ?self.order().uid,
            ?fee_from_surplus,
            ?fee_from_volume,
            ?protocol_fee,
            executed = ?self.executed(),
            surplus_fee = ?self.surplus_fee(),
            "calculated protocol fee"
        );
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
        protocol_fee: &FeePolicy,
    ) -> Result<eth::TokenAmount, Error> {
        let fee_in_sell_token = match self.order().side {
            Side::Buy => self.protocol_fee(prices, protocol_fee)?,
            Side::Sell => self
                .protocol_fee(prices, protocol_fee)?
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
                .checked_sub(
                    quote
                        .fee_amount
                        .checked_mul(quote.buy_amount)
                        .ok_or(Math::Overflow)?
                        .checked_div(quote.sell_amount)
                        .ok_or(Math::DivisionByZero)?,
                )
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
    #[error("orders with non solver determined gas cost fees are not supported")]
    ProtocolFeeOnStaticOrder,
    #[error("protocol fees result in limit price violation")]
    LimitPriceViolatedByProtocolFees,
    #[error(transparent)]
    Math(#[from] Math),
    #[error(transparent)]
    Fulfillment(#[from] Trade),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::competition::{
            self,
            order::{self, FeePolicy, Kind, Partial},
        },
        alloy::primitives::Bytes,
        number::units::EthUnit,
    };

    #[test]
    fn test_adjust_quote_to_out_market_sell_order_limits() {
        let order = Order {
            sell_amount: 20u64.eth(),
            buy_amount: 19u64.eth(),
            side: Side::Sell,
        };
        let quote = Quote {
            sell_amount: 21u64.eth(),
            buy_amount: 18u64.eth(),
            fee_amount: 1u64.eth(),
        };
        let limit = adjust_quote_to_order_limits(order.clone(), quote).unwrap();

        assert_eq!(
            limit.sell.0, order.sell_amount,
            "Sell amount should match order sell amount for sell orders."
        );
        assert_eq!(
            limit.buy.0,
            19u64.eth(),
            "Buy amount should be equal to order buy amount for out of market orders"
        );
    }

    #[test]
    fn test_adjust_quote_to_out_market_buy_order_limits() {
        let order = Order {
            sell_amount: 20u64.eth(),
            buy_amount: 19u64.eth(),
            side: Side::Buy,
        };
        let quote = Quote {
            sell_amount: 21u64.eth(),
            buy_amount: 18u64.eth(),
            fee_amount: 1u64.eth(),
        };

        let limit = adjust_quote_to_order_limits(order.clone(), quote).unwrap();

        assert_eq!(
            limit.buy.0, order.buy_amount,
            "Buy amount should match order buy amount for buy orders."
        );
        assert_eq!(
            limit.sell.0,
            20u64.eth(),
            "Sell amount should be equal to order sell amount for out of market orders."
        );
    }

    #[test]
    fn test_adjust_quote_to_in_market_sell_order_limits() {
        let order = Order {
            sell_amount: 10u64.eth(),
            buy_amount: 10u64.eth(),
            side: Side::Sell,
        };
        let quote = Quote {
            sell_amount: 10u64.eth(),
            buy_amount: 25u64.eth(),
            fee_amount: 2u64.eth(),
        };

        let limit = adjust_quote_to_order_limits(order.clone(), quote.clone()).unwrap();

        assert_eq!(
            limit.sell.0, order.sell_amount,
            "Sell amount should be taken from the order for sell orders in market price."
        );
        assert_eq!(
            limit.buy.0,
            20u64.eth(),
            "Buy amount should be equal to quoted buy amount but reduced by fee."
        );
    }

    #[test]
    fn test_adjust_quote_to_in_market_buy_order_limits() {
        let order = Order {
            sell_amount: 20u64.eth(),
            buy_amount: 10u64.eth(),
            side: Side::Buy,
        };
        let quote = Quote {
            sell_amount: 17u64.eth(),
            buy_amount: 10u64.eth(),
            fee_amount: 1u64.eth(),
        };

        let limit = adjust_quote_to_order_limits(order.clone(), quote.clone()).unwrap();

        assert_eq!(
            limit.sell.0,
            18u64.eth(),
            "Sell amount should match quoted buy amount increased by fee"
        );
        assert_eq!(
            limit.buy.0, order.buy_amount,
            "Buy amount should be taken from the order for buy orders in market price."
        );
    }

    fn sell_order_with_volume_fee(
        sell: eth::U256,
        buy: eth::U256,
        fee_factor: f64,
    ) -> competition::Order {
        let sell_token = eth::TokenAddress::from(eth::Address::random());
        let buy_token = eth::TokenAddress::from(eth::Address::random());
        competition::Order {
            uid: order::Uid::default(),
            receiver: None,
            created: 0u32.into(),
            valid_to: u32::MAX.into(),
            sell: eth::Asset {
                amount: sell.into(),
                token: sell_token,
            },
            buy: eth::Asset {
                amount: buy.into(),
                token: buy_token,
            },
            side: Side::Sell,
            kind: Kind::Limit,
            app_data: Default::default(),
            partial: Partial::No,
            pre_interactions: vec![],
            post_interactions: vec![],
            sell_token_balance: order::SellTokenBalance::Erc20,
            buy_token_balance: order::BuyTokenBalance::Erc20,
            signature: order::Signature {
                scheme: order::signature::Scheme::PreSign,
                data: Bytes::new(),
                signer: eth::Address::default(),
            },
            protocol_fees: vec![FeePolicy::Volume { factor: fee_factor }],
            quote: None,
        }
    }

    /// Volume fee exceeds surplus on a tight stable-to-stable pair → should
    /// be caught before simulation.
    #[test]
    fn volume_fee_violates_limit_price() {
        // Sell 1000 USDC for at least 999.9 USDT (0.01% diff)
        let order = sell_order_with_volume_fee(
            eth::U256::from(1_000_000_000u64),
            eth::U256::from(999_900_000u64), // 999.9e6 buy limit
            0.0002,                          // 2 bps volume fee
        );

        // Solver finds 1:1 route, no solver fee
        let fulfillment = Fulfillment::new(
            order,
            order::TargetAmount(eth::U256::from(1_000_000_000u64)),
            Fee::Dynamic(order::SellAmount(eth::U256::ZERO)),
            eth::U256::ZERO,
        )
        .unwrap();

        let prices = ClearingPrices {
            sell: eth::U256::from(1u64),
            buy: eth::U256::from(1u64),
        };

        let err = fulfillment.with_protocol_fees(prices).unwrap_err();
        assert!(
            matches!(err, Error::LimitPriceViolatedByProtocolFees),
            "expected LimitPriceViolatedByProtocolFees, got {err:?}"
        );
    }

    /// Volume fee within surplus margin → should succeed.
    #[test]
    fn volume_fee_within_surplus() {
        // Sell 1000 USDC for at least 999 USDT (0.1% tolerance)
        let order = sell_order_with_volume_fee(
            eth::U256::from(1_000_000_000u64), // 1000e6 sell
            eth::U256::from(999_000_000u64),   // 999e6 buy limit
            0.0002,                            // 2 bps volume fee (< 0.1% surplus)
        );

        let fulfillment = Fulfillment::new(
            order,
            order::TargetAmount(eth::U256::from(1_000_000_000u64)),
            Fee::Dynamic(order::SellAmount(eth::U256::ZERO)),
            eth::U256::ZERO,
        )
        .unwrap();

        let prices = ClearingPrices {
            sell: eth::U256::from(1u64),
            buy: eth::U256::from(1u64),
        };

        fulfillment
            .with_protocol_fees(prices)
            .expect("fee within surplus should not violate limit price");
    }
}
