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
        trade::{ClearingPrices, Fulfillment},
    },
    crate::domain::competition::{
        PriceLimits,
        order::{self, FeePolicy, Side},
        solution::error::Trade,
    },
    eth_domain_types as eth,
    number::u256_ext::U256Ext,
};

impl Fulfillment {
    /// Applies the protocol fees to the existing fulfillment creating a new
    /// one.
    pub fn with_protocol_fees(&self, prices: ClearingPrices) -> Result<Self, Error> {
        let mut current_fulfillment = self.clone();
        for protocol_fee in &self.order().protocol_fees {
            current_fulfillment = current_fulfillment.with_protocol_fee(prices, protocol_fee)?;
        }
        Ok(current_fulfillment)
    }

    /// Applies the protocol fee to the existing fulfillment creating a new one.
    fn with_protocol_fee(
        &self,
        prices: ClearingPrices,
        protocol_fee: &FeePolicy,
    ) -> Result<Self, Error> {
        let surplus_fee = match protocol_fees::fee_before_settlement(
            &self.fill_at(&prices)?,
            &protocol_fee.into(),
        ) {
            Ok(fee) => fee,
            // A fill under its limit price has no surplus, and a fill with
            // nothing executed has no legs to measure its surplus or price
            // improvement by. The settlement simulation rejects such fills.
            Err(chain_types::MathError::Negative)
                if matches!(protocol_fee, FeePolicy::Surplus { .. }) =>
            {
                eth::U256::ZERO
            }
            Err(chain_types::MathError::DivisionByZero) if self.executed().0.is_zero() => {
                eth::U256::ZERO
            }
            Err(err) => return Err(Math::from(err).into()),
        };
        tracing::debug!(
            uid = ?self.order().uid,
            protocol_fee = ?surplus_fee,
            executed = ?self.executed(),
            fee = ?self.fee(),
            "calculated protocol fee"
        );
        let protocol_fee = self.fee_in_sell_token(prices, eth::TokenAmount(surplus_fee))?;

        // Increase the fee by the protocol fee
        let fee = order::SellAmount(
            self.fee()
                .0
                .checked_add(protocol_fee.0)
                .ok_or(Math::Overflow)?,
        );

        // Reduce the executed amount by the protocol fee. This is because
        // solvers are unaware of the protocol fee that driver
        // introduces and they only account for their own fee.
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

    /// The fill as `protocol_fees` sees it: the signed limit and the legs
    /// the fulfillment settles at `prices`, the fee inside the sell leg.
    fn fill_at(&self, prices: &ClearingPrices) -> Result<protocol_fees::Order, Math> {
        let order = self.order();
        Ok(protocol_fees::Order {
            uid: chain_types::evm::OrderUid(order.uid.0.0),
            sell_token: order.sell.token.into(),
            buy_token: order.buy.token.into(),
            sell_amount: order.sell.amount.0,
            buy_amount: order.buy.amount.0,
            executed_sell: self.sell_amount(prices)?.0,
            executed_buy: self.buy_amount(prices)?.0,
            side: match order.side {
                Side::Sell => protocol_fees::Side::Sell,
                Side::Buy => protocol_fees::Side::Buy,
            },
        })
    }

    /// Converts a fee in the surplus token into the sell token at `prices`.
    fn fee_in_sell_token(
        &self,
        prices: ClearingPrices,
        fee: eth::TokenAmount,
    ) -> Result<eth::TokenAmount, Error> {
        Ok(match self.order().side {
            Side::Buy => fee,
            Side::Sell => fee
                .0
                .checked_mul_ratio(&prices.buy, &prices.sell)
                .map_err(Math::from)?
                .into(),
        })
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
    #[error(transparent)]
    Math(#[from] Math),
    #[error(transparent)]
    Fulfillment(#[from] Trade),
}

// todo: should be removed once integration tests are implemented
#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            domain::competition::{self, order::signature},
            util,
        },
        number::units::EthUnit,
        rand::{Rng, SeedableRng, rngs::StdRng},
        std::sync::Arc,
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

    /// The fee formulas written directly over the fulfillment and the uniform
    /// prices, the oracle for the equality test below.
    mod legacy {
        use super::*;

        pub fn with_protocol_fees(
            fulfillment: &Fulfillment,
            prices: ClearingPrices,
        ) -> Result<Fulfillment, Error> {
            let mut current_fulfillment = fulfillment.clone();
            for protocol_fee in &fulfillment.order().protocol_fees {
                current_fulfillment =
                    with_protocol_fee(&current_fulfillment, prices, protocol_fee)?;
            }
            Ok(current_fulfillment)
        }

        fn with_protocol_fee(
            fulfillment: &Fulfillment,
            prices: ClearingPrices,
            protocol_fee: &FeePolicy,
        ) -> Result<Fulfillment, Error> {
            let protocol_fee = protocol_fee_in_sell_token(fulfillment, prices, protocol_fee)?;
            let fee = order::SellAmount(
                fulfillment
                    .fee()
                    .0
                    .checked_add(protocol_fee.0)
                    .ok_or(Math::Overflow)?,
            );
            let order = fulfillment.order().clone();
            let executed = match order.side {
                order::Side::Buy => fulfillment.executed(),
                order::Side::Sell => order::TargetAmount(
                    fulfillment
                        .executed()
                        .0
                        .checked_sub(protocol_fee.0)
                        .ok_or(Math::Overflow)?,
                ),
            };
            Fulfillment::new(order, executed, fee).map_err(Into::into)
        }

        fn protocol_fee(
            fulfillment: &Fulfillment,
            prices: ClearingPrices,
            protocol_fee: &FeePolicy,
        ) -> Result<eth::TokenAmount, Error> {
            match protocol_fee {
                FeePolicy::Surplus {
                    factor,
                    max_volume_factor,
                } => calculate_fee(
                    fulfillment,
                    PriceLimits {
                        sell: fulfillment.order().sell.amount,
                        buy: fulfillment.order().buy.amount,
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
                            sell_amount: fulfillment.order().sell.amount.0,
                            buy_amount: fulfillment.order().buy.amount.0,
                            side: fulfillment.order().side,
                        },
                        Quote {
                            sell_amount: quote.sell.amount.0,
                            buy_amount: quote.buy.amount.0,
                            fee_amount: quote.fee.amount.0,
                        },
                    )?;
                    calculate_fee(
                        fulfillment,
                        price_limits,
                        prices,
                        *factor,
                        *max_volume_factor,
                    )
                }
                FeePolicy::Volume { factor, .. } => fee_from_volume(fulfillment, prices, *factor),
            }
        }

        fn calculate_fee(
            fulfillment: &Fulfillment,
            price_limits: PriceLimits,
            prices: ClearingPrices,
            factor: f64,
            max_volume_factor: f64,
        ) -> Result<eth::TokenAmount, Error> {
            let fee_from_surplus = fee_from_surplus(
                fulfillment,
                price_limits.sell.0,
                price_limits.buy.0,
                prices,
                factor,
            )?;
            let fee_from_volume = fee_from_volume(fulfillment, prices, max_volume_factor)?;
            Ok(std::cmp::min(fee_from_surplus, fee_from_volume))
        }

        fn fee_from_surplus(
            fulfillment: &Fulfillment,
            sell_amount: eth::U256,
            buy_amount: eth::U256,
            prices: ClearingPrices,
            factor: f64,
        ) -> Result<eth::TokenAmount, Error> {
            let surplus =
                surplus_over_reference_price(fulfillment, sell_amount, buy_amount, prices)?;
            surplus
                .apply_factor(factor)
                .ok_or(Math::Overflow)
                .map_err(Into::into)
        }

        fn fee_from_volume(
            fulfillment: &Fulfillment,
            prices: ClearingPrices,
            factor: f64,
        ) -> Result<eth::TokenAmount, Error> {
            let volume = match fulfillment.order().side {
                Side::Buy => fulfillment.sell_amount(&prices)?,
                Side::Sell => fulfillment.buy_amount(&prices)?,
            };
            volume
                .apply_factor(factor)
                .ok_or(Math::Overflow)
                .map_err(Into::into)
        }

        fn protocol_fee_in_sell_token(
            fulfillment: &Fulfillment,
            prices: ClearingPrices,
            policy: &FeePolicy,
        ) -> Result<eth::TokenAmount, Error> {
            let fee_in_sell_token = match fulfillment.order().side {
                Side::Buy => protocol_fee(fulfillment, prices, policy)?,
                Side::Sell => protocol_fee(fulfillment, prices, policy)?
                    .0
                    .checked_mul_ratio(&prices.buy, &prices.sell)
                    .map_err(Math::from)?
                    .into(),
            };
            Ok(fee_in_sell_token)
        }

        fn surplus_over_reference_price(
            fulfillment: &Fulfillment,
            limit_sell: eth::U256,
            limit_buy: eth::U256,
            prices: ClearingPrices,
        ) -> Result<eth::TokenAmount, Trade> {
            let executed = fulfillment.executed().0;
            let executed_sell_amount = match fulfillment.order().side {
                Side::Buy => executed
                    .checked_mul(prices.buy)
                    .ok_or(Math::Overflow)?
                    .checked_div(prices.sell)
                    .ok_or(Math::DivisionByZero)?,
                Side::Sell => executed,
            };
            let executed_sell_amount_with_fee = executed_sell_amount
                .checked_add(fulfillment.fee().0)
                .ok_or(Math::Overflow)?;
            let surplus = match fulfillment.order().side {
                Side::Buy => {
                    let limit_sell_amount = limit_sell
                        .checked_mul(executed)
                        .ok_or(Math::Overflow)?
                        .checked_div(limit_buy)
                        .ok_or(Math::DivisionByZero)?;
                    limit_sell_amount
                        .checked_sub(executed_sell_amount_with_fee)
                        .unwrap_or(eth::U256::ZERO)
                }
                Side::Sell => {
                    let limit_buy_amount = limit_buy
                        .checked_mul(executed_sell_amount_with_fee)
                        .ok_or(Math::Overflow)?
                        .checked_ceil_div(&limit_sell)
                        .ok_or(Math::DivisionByZero)?;
                    let executed_buy_amount = executed
                        .checked_mul(prices.sell)
                        .ok_or(Math::Overflow)?
                        .checked_ceil_div(&prices.buy)
                        .ok_or(Math::DivisionByZero)?;
                    executed_buy_amount
                        .checked_sub(limit_buy_amount)
                        .unwrap_or(eth::U256::ZERO)
                }
            };
            Ok(surplus.into())
        }
    }

    fn sell(amount: u64) -> eth::Asset {
        eth::Asset {
            token: eth::Address::left_padding_from(0x5e11_u64.to_be_bytes().as_slice()).into(),
            amount: eth::U256::from(amount).into(),
        }
    }

    fn buy(amount: u64) -> eth::Asset {
        eth::Asset {
            token: eth::Address::left_padding_from(0xbbbb_u64.to_be_bytes().as_slice()).into(),
            amount: eth::U256::from(amount).into(),
        }
    }

    /// A partially fillable order with its whole target available.
    fn order(
        sell_amount: u64,
        buy_amount: u64,
        side: Side,
        protocol_fees: Vec<FeePolicy>,
    ) -> competition::Order {
        competition::Order {
            data: Arc::new(order::OrderData {
                uid: Default::default(),
                receiver: Default::default(),
                created: util::Timestamp(100),
                valid_to: util::Timestamp(u32::MAX),
                buy: buy(buy_amount),
                sell: sell(sell_amount),
                side,
                pre_interactions: Default::default(),
                post_interactions: Default::default(),
                sell_token_balance: order::SellTokenBalance::Erc20,
                buy_token_balance: order::BuyTokenBalance::Erc20,
                signature: order::Signature {
                    scheme: signature::Scheme::PreSign,
                    data: Default::default(),
                    signer: Default::default(),
                },
                protocol_fees,
                quote: Default::default(),
                penalty_cap_native: Default::default(),
            }),
            app_data: Default::default(),
            partial: order::Partial::Yes {
                available: order::TargetAmount(eth::U256::from(match side {
                    Side::Sell => sell_amount,
                    Side::Buy => buy_amount,
                })),
            },
        }
    }

    fn random_policy(rng: &mut StdRng) -> FeePolicy {
        let factor = rng.random_range(0.0..=0.5);
        let max_volume_factor = rng.random_range(0.0..=0.5);
        match rng.random_range(0..3) {
            0 => FeePolicy::Volume {
                factor,
                contributes_to_score: rng.random_bool(0.5),
            },
            1 => FeePolicy::Surplus {
                factor,
                max_volume_factor,
            },
            _ => {
                let sell_amount = rng.random_range(1..=1u64 << 48);
                FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                    quote: order::Quote {
                        sell: sell(sell_amount),
                        buy: buy(rng.random_range(1..=1u64 << 48)),
                        fee: sell(rng.random_range(0..=sell_amount / 2)),
                        solver: eth::Address::repeat_byte(9),
                    },
                }
            }
        }
    }

    /// A partially filled fulfillment on either side with a fee already in it,
    /// at uniform prices that mostly put it over its limit and sometimes
    /// under, the legs scaled by a random factor plus a jitter below that
    /// factor, so deriving the legs back has to round. A few buy fills
    /// execute nothing.
    fn random_fulfillment(rng: &mut StdRng) -> (Fulfillment, ClearingPrices) {
        let side = if rng.random_bool(0.5) {
            Side::Sell
        } else {
            Side::Buy
        };
        let sell_amount = rng.random_range(1..=1u64 << 48);
        let buy_amount = rng.random_range(1..=1u64 << 48);
        let policies = (0..rng.random_range(0..=3))
            .map(|_| random_policy(rng))
            .collect();
        let order = order(sell_amount, buy_amount, side, policies);
        let (executed, fee) = match side {
            Side::Sell => {
                let total = rng.random_range(1..=sell_amount);
                let fee = rng.random_range(0..total);
                (total - fee, fee)
            }
            Side::Buy => (
                if rng.random_bool(0.02) {
                    0
                } else {
                    rng.random_range(1..=buy_amount)
                },
                rng.random_range(0..=sell_amount / 10),
            ),
        };
        let scale = rng.random_range(1..=1_000u64);
        let jitter = rng.random_range(0..scale);
        let surplus = rng.random_range(0..=1u64 << 47);
        let wide = |amount: u64| u128::from(amount);
        let prices = match side {
            Side::Sell => {
                let at_limit =
                    (wide(buy_amount) * wide(executed + fee)).div_ceil(wide(sell_amount));
                let bought = at_limit - rng.random_range(0..=at_limit / 50) + wide(surplus);
                ClearingPrices {
                    sell: eth::U256::from(bought * wide(scale) + wide(jitter)),
                    buy: eth::U256::from(wide(executed) * wide(scale)),
                }
            }
            Side::Buy => {
                let limit = wide(sell_amount) * wide(executed) / wide(buy_amount);
                let sold = limit
                    .saturating_sub(wide(fee))
                    .saturating_sub(wide(surplus))
                    .max(1);
                ClearingPrices {
                    sell: eth::U256::from(wide(executed.max(1)) * wide(scale)),
                    buy: eth::U256::from(sold * wide(scale) + wide(jitter)),
                }
            }
        };
        let fulfillment = Fulfillment::new(
            order,
            order::TargetAmount(eth::U256::from(executed)),
            order::SellAmount(eth::U256::from(fee)),
        )
        .unwrap();
        (fulfillment, prices)
    }

    fn has_surplus_policy(fulfillment: &Fulfillment) -> bool {
        fulfillment
            .order()
            .protocol_fees
            .iter()
            .any(|policy| matches!(policy, FeePolicy::Surplus { .. }))
    }

    /// Whether the settlement contract rejects the fill at these prices.
    fn under_limit(fulfillment: &Fulfillment, prices: ClearingPrices) -> bool {
        let order = fulfillment.order();
        let sold = fulfillment.sell_amount(&prices).unwrap().0;
        let bought = fulfillment.buy_amount(&prices).unwrap().0;
        order.sell.amount.0 * bought < order.buy.amount.0 * sold
    }

    /// The generic math gives the legacy formulas' numbers wherever those
    /// produce one.
    #[test]
    fn fees_match_the_legacy_formulas() {
        let mut rng = StdRng::seed_from_u64(0x5eed);
        let mut compared = Vec::new();
        let mut under_limit_with_surplus_policy = 0;
        let mut nothing_executed = 0;
        for _ in 0..5_000 {
            let (fulfillment, prices) = random_fulfillment(&mut rng);
            match (
                legacy::with_protocol_fees(&fulfillment, prices),
                fulfillment.with_protocol_fees(prices),
            ) {
                (Ok(expected), Ok(actual)) => {
                    assert_eq!(actual.executed(), expected.executed(), "{fulfillment:?}");
                    assert_eq!(actual.fee(), expected.fee(), "{fulfillment:?}");
                    if has_surplus_policy(&fulfillment) && under_limit(&fulfillment, prices) {
                        under_limit_with_surplus_policy += 1;
                    }
                    if fulfillment.executed().0.is_zero() {
                        nothing_executed += 1;
                    }
                    compared.push((
                        fulfillment.order().side,
                        fulfillment.order().protocol_fees.is_empty(),
                    ));
                }
                (Ok(_), Err(err)) => panic!("refused {err:?} {fulfillment:?}"),
                (Err(err), Ok(_)) => panic!("legacy refused {err:?} {fulfillment:?}"),
                (Err(_), Err(_)) => {}
            }
        }
        assert!(
            under_limit_with_surplus_policy > 0,
            "no fill under its limit met a surplus policy"
        );
        assert!(nothing_executed > 0, "no fill executed nothing");
        for side in [Side::Sell, Side::Buy] {
            for with_policies in [true, false] {
                let count = compared
                    .iter()
                    .filter(|(s, no_policies)| *s == side && *no_policies != with_policies)
                    .count();
                assert!(
                    count > 200,
                    "{side:?} with_policies={with_policies}: {count} fulfillments compared"
                );
            }
        }
    }

    /// A surplus policy takes nothing from a fill under its limit price. The
    /// settlement simulation rejects the fill.
    #[test]
    fn a_fill_under_its_limit_takes_no_surplus_fee() {
        let order = order(
            1_000,
            2_000,
            Side::Sell,
            vec![FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 0.1,
            }],
        );
        let fulfillment = Fulfillment::new(
            order,
            order::TargetAmount(eth::U256::from(1_000)),
            order::SellAmount(eth::U256::ZERO),
        )
        .unwrap();
        let prices = ClearingPrices {
            sell: eth::U256::from(1_900),
            buy: eth::U256::from(1_000),
        };
        let applied = fulfillment.with_protocol_fees(prices).unwrap();
        assert_eq!(applied.executed(), fulfillment.executed());
        assert_eq!(applied.fee(), fulfillment.fee());
    }

    /// A fill with nothing executed has no legs to measure its surplus by, so
    /// a surplus policy takes nothing. The settlement simulation rejects the
    /// fill.
    #[test]
    fn a_fill_with_nothing_executed_takes_no_surplus_fee() {
        let order = order(
            1_000,
            2_000,
            Side::Buy,
            vec![FeePolicy::Surplus {
                factor: 0.5,
                max_volume_factor: 0.1,
            }],
        );
        let fulfillment = Fulfillment::new(
            order,
            order::TargetAmount(eth::U256::ZERO),
            order::SellAmount(eth::U256::from(7)),
        )
        .unwrap();
        let prices = ClearingPrices {
            sell: eth::U256::from(2),
            buy: eth::U256::from(1),
        };
        let applied = fulfillment.with_protocol_fees(prices).unwrap();
        assert_eq!(applied.executed(), fulfillment.executed());
        assert_eq!(applied.fee(), fulfillment.fee());
    }
}
