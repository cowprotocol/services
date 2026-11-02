pub use error::Error;
use {
    super::ExecutedProtocolFee,
    crate::domain::{
        self,
        OrderUid,
        auction::{self, order},
        fee,
        settlement::transaction::{ClearingPrices, Prices},
    },
    error::Math,
    eth_domain_types as eth,
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
    /// The fill as the generic fee math sees it: the signed limit and the
    /// legs `prices` settle it at. The settlement contract rounds the bought
    /// amount up.
    pub fn fill_at(&self, prices: &ClearingPrices) -> Result<protocol_fees::Order, Math> {
        let (executed_sell, executed_buy) = match self.side {
            order::Side::Sell => (
                self.executed.0,
                self.executed
                    .0
                    .checked_mul_ratio_ceil(&prices.sell, &prices.buy)?,
            ),
            order::Side::Buy => (
                self.executed
                    .0
                    .checked_mul_ratio(&prices.buy, &prices.sell)?,
                self.executed.0,
            ),
        };
        Ok(protocol_fees::Order {
            uid: chain_types::evm::OrderUid(self.uid.0),
            sell_token: *self.sell.token,
            buy_token: *self.buy.token,
            sell_amount: self.sell.amount.0,
            buy_amount: self.buy.amount.0,
            executed_sell,
            executed_buy,
            side: match self.side {
                order::Side::Buy => protocol_fees::Side::Buy,
                order::Side::Sell => protocol_fees::Side::Sell,
            },
        })
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
                .checked_mul_ratio(&self.prices.uniform.buy, &self.prices.uniform.sell)
                .map_err(error::Math::from)?,
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
        let generic_policies: Vec<protocol_fees::FeePolicy> =
            policies.iter().copied().map(Into::into).collect();
        let fees = protocol_fees::protocol_fees_by_policy(
            &self.fill_at(&self.prices.custom)?,
            &generic_policies,
        )
        .map_err(Math::from)?;
        Ok(policies
            .iter()
            .zip(fees)
            .map(|(policy, fee)| ExecutedProtocolFee {
                policy: *policy,
                fee: eth::Asset {
                    token: self.surplus_token(),
                    amount: eth::TokenAmount(fee),
                },
            })
            .collect())
    }

    /// Uses custom prices to calculate the surplus after the protocol fee and
    /// network fee are applied.
    fn surplus_over_limit_price(&self) -> Result<eth::SurplusTokenAmount, error::Math> {
        let surplus = protocol_fees::surplus_over_limit(&self.fill_at(&self.prices.custom)?)?;
        Ok(eth::SurplusTokenAmount(surplus))
    }

    /// Uses uniform prices to calculate the surplus as if the protocol fee and
    /// network fee are not applied.
    fn surplus_over_limit_price_before_fee(&self) -> Result<eth::SurplusTokenAmount, error::Math> {
        let surplus = protocol_fees::surplus_over_limit(&self.fill_at(&self.prices.uniform)?)?;
        Ok(eth::SurplusTokenAmount(surplus))
    }

    fn surplus_token(&self) -> eth::TokenAddress {
        match self.side {
            order::Side::Buy => self.sell.token,
            order::Side::Sell => self.buy.token,
        }
    }
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
    use eth_domain_types as eth;

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

    impl From<number::MathError> for Math {
        fn from(err: number::MathError) -> Self {
            match err {
                number::MathError::DivisionByZero => Self::DivisionByZero,
                number::MathError::Overflow => Self::Overflow,
            }
        }
    }

    impl From<chain_types::MathError> for Math {
        fn from(err: chain_types::MathError) -> Self {
            match err {
                chain_types::MathError::Overflow => Self::Overflow,
                chain_types::MathError::DivisionByZero => Self::DivisionByZero,
                chain_types::MathError::Negative => Self::Negative,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        configs::fee_factor::FeeFactor,
        rand::{Rng, SeedableRng, rngs::StdRng},
        std::str::FromStr,
    };

    /// The trade from prod xdai settlement
    /// 0xb02d542de9a45dfad68d152cfede1fa0c322b988ecc6be4b398c0f9c6b1048c1
    /// (auction 16489395), whose solver-encoded uniform clearing prices of
    /// magnitude 1e60 made `amount * price` exceed 256 bits. The old math
    /// overflowed, the whole fee calculation bailed, and the execution was
    /// persisted with zeroed fees.
    fn incident_trade(side: order::Side) -> Trade {
        let prices = ClearingPrices {
            sell: eth::U256::from_str(
                "1457533905173705573868216339202397616043789250000000000000000",
            )
            .unwrap(),
            buy: eth::U256::from_str(
                "1457533885739920430673304276561480416152609058094152638608893",
            )
            .unwrap(),
        };
        Trade {
            uid: domain::OrderUid([0; 56]),
            sell: eth::Asset {
                amount: eth::U256::from(80502414430363770111_u128).into(),
                token: eth::Address::default().into(),
            },
            buy: eth::Asset {
                amount: eth::U256::from(80500000000000000000_u128).into(),
                token: eth::Address::default().into(),
            },
            side,
            executed: order::TargetAmount(eth::U256::from(80500000000000000000_u128)),
            prices: Prices {
                uniform: prices,
                custom: prices,
            },
        }
    }

    #[test]
    fn surplus_survives_hugely_scaled_clearing_prices() {
        let trade = incident_trade(order::Side::Buy);
        let surplus = trade.surplus_over_limit_price_before_fee().unwrap();
        assert_eq!(surplus.0, eth::U256::from(2415503697089133_u64));
    }

    /// Sell side hits the fee-to-sell-token conversion, which multiplies the
    /// fee by the uniform buy price and overflowed the same way.
    #[test]
    fn fee_conversion_survives_hugely_scaled_clearing_prices() {
        let fee = incident_trade(order::Side::Sell)
            .fee_into_sell_token(eth::SurplusTokenAmount(eth::U256::from(
                1_000_000_000_000_000_000_u128,
            )))
            .unwrap();
        assert_eq!(fee.0, eth::U256::from(999999986666666844_u64));
    }

    /// The autopilot's own fee formulas, kept verbatim as the oracle for the
    /// equality test below.
    mod legacy {
        use {
            super::*,
            num::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub},
        };

        pub struct PriceLimits {
            pub sell: eth::TokenAmount,
            pub buy: eth::TokenAmount,
        }

        struct Order {
            sell: eth::TokenAmount,
            buy: eth::TokenAmount,
            side: order::Side,
        }

        struct Quote {
            sell: eth::TokenAmount,
            buy: eth::TokenAmount,
            fee: eth::TokenAmount,
        }

        fn surplus_over(
            trade: &Trade,
            prices: &ClearingPrices,
            price_limits: PriceLimits,
        ) -> Result<eth::SurplusTokenAmount, Math> {
            match trade.side {
                order::Side::Buy => {
                    let limit_sell = price_limits
                        .sell
                        .0
                        .checked_mul_ratio(&trade.executed.into(), &price_limits.buy.0)?;
                    let sold = trade
                        .executed
                        .0
                        .checked_mul_ratio(&prices.buy, &prices.sell)?;
                    limit_sell.checked_sub(sold).ok_or(Math::Negative)
                }
                order::Side::Sell => {
                    let limit_buy = trade
                        .executed
                        .0
                        .checked_mul_ratio_ceil(&price_limits.buy.0, &price_limits.sell.0)?;
                    let bought = trade
                        .executed
                        .0
                        .checked_mul_ratio_ceil(&prices.sell, &prices.buy)?;
                    bought.checked_sub(limit_buy).ok_or(Math::Negative)
                }
            }
            .map(eth::SurplusTokenAmount)
        }

        pub fn surplus_over_limit_price(trade: &Trade) -> Result<eth::SurplusTokenAmount, Math> {
            let limit_price = PriceLimits {
                sell: trade.sell.amount,
                buy: trade.buy.amount,
            };
            surplus_over(trade, &trade.prices.custom, limit_price)
        }

        pub fn surplus_over_limit_price_before_fee(
            trade: &Trade,
        ) -> Result<eth::SurplusTokenAmount, Math> {
            let limit_price = PriceLimits {
                sell: trade.sell.amount,
                buy: trade.buy.amount,
            };
            surplus_over(trade, &trade.prices.uniform, limit_price)
        }

        pub fn protocol_fees(
            trade: &Trade,
            fee_policies: &HashMap<OrderUid, impl AsRef<[fee::Policy]>>,
        ) -> Result<Vec<ExecutedProtocolFee>, Error> {
            let policies = fee_policies
                .get(&trade.uid)
                .map(|value| value.as_ref())
                .unwrap_or_default();
            let mut current_trade = trade.clone();
            let mut total = eth::SurplusTokenAmount::default();
            let mut fees = vec![];
            for (i, policy) in policies.iter().enumerate().rev() {
                let fee = protocol_fee(&current_trade, policy)?;
                fees.push(ExecutedProtocolFee {
                    policy: *policy,
                    fee: eth::Asset {
                        token: trade.surplus_token(),
                        amount: eth::TokenAmount(fee.0),
                    },
                });
                total = eth::SurplusTokenAmount(
                    total
                        .0
                        .checked_add(fee.0)
                        .ok_or(Error::Math(Math::Overflow))?,
                );
                if i != 0 {
                    current_trade.prices.custom = calculate_custom_prices(trade, total)?;
                }
            }
            fees.reverse();
            Ok(fees)
        }

        fn sell_amount(trade: &Trade) -> Result<eth::TokenAmount, Math> {
            Ok(match trade.side {
                order::Side::Sell => trade.executed.0,
                order::Side::Buy => trade
                    .executed
                    .0
                    .checked_mul_ratio(&trade.prices.custom.buy, &trade.prices.custom.sell)?,
            }
            .into())
        }

        fn buy_amount(trade: &Trade) -> Result<eth::TokenAmount, Math> {
            Ok(match trade.side {
                order::Side::Sell => trade
                    .executed
                    .0
                    .checked_mul_ratio_ceil(&trade.prices.custom.sell, &trade.prices.custom.buy)?,
                order::Side::Buy => trade.executed.0,
            }
            .into())
        }

        fn calculate_custom_prices(
            trade: &Trade,
            protocol_fee: eth::SurplusTokenAmount,
        ) -> Result<ClearingPrices, Math> {
            Ok(ClearingPrices {
                sell: match trade.side {
                    order::Side::Sell => buy_amount(trade)?
                        .0
                        .checked_add(protocol_fee.0)
                        .ok_or(Math::Overflow)?,
                    order::Side::Buy => buy_amount(trade)?.0,
                },
                buy: match trade.side {
                    order::Side::Sell => sell_amount(trade)?.0,
                    order::Side::Buy => sell_amount(trade)?
                        .0
                        .checked_sub(protocol_fee.0)
                        .ok_or(Math::Negative)?,
                },
            })
        }

        fn protocol_fee(
            trade: &Trade,
            fee_policy: &fee::Policy,
        ) -> Result<eth::SurplusTokenAmount, Error> {
            let fee = match fee_policy {
                fee::Policy::Surplus {
                    factor,
                    max_volume_factor,
                } => {
                    let surplus = surplus_over_limit_price(trade)?;
                    std::cmp::min(
                        surplus_fee(surplus, (*factor).get())?,
                        volume_fee(trade, (*max_volume_factor).get())?,
                    )
                }
                fee::Policy::PriceImprovement {
                    factor,
                    max_volume_factor,
                    quote,
                } => {
                    let price_improvement = price_improvement(trade, quote)?;
                    std::cmp::min(
                        surplus_fee(price_improvement, (*factor).get())?,
                        volume_fee(trade, (*max_volume_factor).get())?,
                    )
                }
                fee::Policy::Volume { factor } => volume_fee(trade, (*factor).get())?,
            };
            Ok(fee)
        }

        fn price_improvement(
            trade: &Trade,
            quote: &domain::fee::Quote,
        ) -> Result<eth::SurplusTokenAmount, Error> {
            let surplus = surplus_over_quote(trade, quote);
            if let Err(Math::Negative) = surplus {
                return Ok(eth::SurplusTokenAmount(eth::U256::ZERO));
            }
            Ok(surplus?)
        }

        fn surplus_over_quote(
            trade: &Trade,
            quote: &domain::fee::Quote,
        ) -> Result<eth::SurplusTokenAmount, Math> {
            let quote = adjust_quote_to_order_limits(
                Order {
                    sell: trade.sell.amount,
                    buy: trade.buy.amount,
                    side: trade.side,
                },
                Quote {
                    sell: quote.sell_amount.into(),
                    buy: quote.buy_amount.into(),
                    fee: quote.fee.into(),
                },
            )?;
            surplus_over(trade, &trade.prices.custom, quote)
        }

        fn surplus_fee(
            surplus: eth::SurplusTokenAmount,
            factor: f64,
        ) -> Result<eth::SurplusTokenAmount, Error> {
            let fee = surplus
                .0
                .checked_mul_f64(factor / (1.0 - factor))
                .ok_or(Math::Overflow)?;
            Ok(eth::SurplusTokenAmount(fee))
        }

        fn volume_fee(trade: &Trade, factor: f64) -> Result<eth::SurplusTokenAmount, Error> {
            let executed_in_surplus_token = match trade.side {
                order::Side::Buy => sell_amount(trade)?,
                order::Side::Sell => buy_amount(trade)?,
            };
            let factor = match trade.side {
                order::Side::Sell => factor / (1.0 - factor),
                order::Side::Buy => factor / (1.0 + factor),
            };
            Ok(eth::SurplusTokenAmount(
                executed_in_surplus_token
                    .0
                    .checked_mul_f64(factor)
                    .ok_or(Math::Overflow)?,
            ))
        }

        fn adjust_quote_to_order_limits(order: Order, quote: Quote) -> Result<PriceLimits, Math> {
            match order.side {
                order::Side::Sell => {
                    let quote_buy_amount = quote
                        .buy
                        .checked_sub(
                            &quote
                                .fee
                                .checked_mul(&quote.buy)
                                .ok_or(Math::Overflow)?
                                .checked_div(&quote.sell)
                                .ok_or(Math::DivisionByZero)?,
                        )
                        .ok_or(Math::Negative)?;
                    let scaled_buy_amount = quote_buy_amount
                        .checked_mul(&order.sell)
                        .ok_or(Math::Overflow)?
                        .checked_div(&quote.sell)
                        .ok_or(Math::DivisionByZero)?;
                    let buy_amount = order.buy.max(scaled_buy_amount);
                    Ok(PriceLimits {
                        sell: order.sell,
                        buy: buy_amount,
                    })
                }
                order::Side::Buy => {
                    let quote_sell_amount =
                        quote.sell.checked_add(&quote.fee).ok_or(Math::Overflow)?;
                    let scaled_sell_amount = quote_sell_amount
                        .checked_mul(&order.buy)
                        .ok_or(Math::Overflow)?
                        .checked_div(&quote.buy)
                        .ok_or(Math::DivisionByZero)?;
                    let sell_amount = order.sell.min(scaled_sell_amount);
                    Ok(PriceLimits {
                        sell: sell_amount,
                        buy: order.buy,
                    })
                }
            }
        }
    }

    fn asset(amount: u64) -> eth::Asset {
        eth::Asset {
            amount: eth::TokenAmount(eth::U256::from(amount)),
            token: eth::Address::default().into(),
        }
    }

    fn random_policy(rng: &mut StdRng) -> fee::Policy {
        let factor = FeeFactor::try_from(rng.random_range(0.0..=0.5)).unwrap();
        let max_volume_factor = FeeFactor::try_from(rng.random_range(0.0..=0.5)).unwrap();
        match rng.random_range(0..3) {
            0 => fee::Policy::Volume { factor },
            1 => fee::Policy::Surplus {
                factor,
                max_volume_factor,
            },
            _ => {
                let sell_amount = rng.random_range(1..=1u64 << 48);
                fee::Policy::PriceImprovement {
                    factor,
                    max_volume_factor,
                    quote: fee::Quote {
                        sell_amount: eth::U256::from(sell_amount),
                        buy_amount: eth::U256::from(rng.random_range(1..=1u64 << 48)),
                        fee: eth::U256::from(rng.random_range(0..=sell_amount / 2)),
                        solver: eth::Address::repeat_byte(9),
                    },
                }
            }
        }
    }

    /// A trade on either side with both price vectors at or over the limit,
    /// the uniform one at least as good for the user as the custom one, each
    /// as its legs scaled by a random factor.
    fn random_trade(rng: &mut StdRng) -> Trade {
        let side = if rng.random_bool(0.5) {
            order::Side::Sell
        } else {
            order::Side::Buy
        };
        let sell_amount = rng.random_range(1..=1u64 << 48);
        let buy_amount = rng.random_range(1..=1u64 << 48);
        let executed = match side {
            order::Side::Sell => rng.random_range(1..=sell_amount),
            order::Side::Buy => rng.random_range(1..=buy_amount),
        };
        let legs = |surplus: u64| match side {
            order::Side::Sell => {
                let at_limit = (u128::from(executed) * u128::from(buy_amount))
                    .div_ceil(u128::from(sell_amount)) as u64;
                (executed, at_limit + surplus)
            }
            order::Side::Buy => {
                let at_limit = (u128::from(executed) * u128::from(sell_amount)
                    / u128::from(buy_amount)) as u64;
                (at_limit.saturating_sub(surplus).max(1), executed)
            }
        };
        let custom_surplus = rng.random_range(0..=1u64 << 47);
        let uniform_surplus = custom_surplus + rng.random_range(0..=1u64 << 47);
        let mut prices = |(executed_sell, executed_buy): (u64, u64)| {
            let scale = eth::U256::from(rng.random_range(1..=1_000u64));
            ClearingPrices {
                sell: eth::U256::from(executed_buy) * scale,
                buy: eth::U256::from(executed_sell) * scale,
            }
        };
        Trade {
            uid: domain::OrderUid([0; 56]),
            sell: asset(sell_amount),
            buy: asset(buy_amount),
            side,
            executed: order::TargetAmount(eth::U256::from(executed)),
            prices: Prices {
                uniform: prices(legs(uniform_surplus)),
                custom: prices(legs(custom_surplus)),
            },
        }
    }

    /// The generic math gives the legacy formulas' numbers wherever those
    /// produce one.
    #[test]
    fn fees_and_surplus_match_the_legacy_formulas() {
        let mut rng = StdRng::seed_from_u64(0x5eed);
        let mut compared = Vec::new();
        for _ in 0..5_000 {
            let trade = random_trade(&mut rng);
            let policies: Vec<fee::Policy> = (0..rng.random_range(0..=3))
                .map(|_| random_policy(&mut rng))
                .collect();
            let fee_policies = HashMap::from([(trade.uid, policies.clone())]);
            let (Ok(fees), Ok(after_fees), Ok(before_fees)) = (
                legacy::protocol_fees(&trade, &fee_policies),
                legacy::surplus_over_limit_price(&trade),
                legacy::surplus_over_limit_price_before_fee(&trade),
            ) else {
                continue;
            };
            compared.push((trade.side, policies.is_empty()));
            let actual = trade.protocol_fees(&fee_policies).unwrap();
            assert_eq!(actual.len(), fees.len(), "{trade:?}");
            for (actual, expected) in actual.iter().zip(&fees) {
                assert_eq!(actual.policy, expected.policy, "{trade:?}");
                assert_eq!(actual.fee, expected.fee, "{trade:?}");
            }
            assert_eq!(
                trade.surplus_over_limit_price().unwrap(),
                after_fees,
                "{trade:?}"
            );
            assert_eq!(
                trade.surplus_over_limit_price_before_fee().unwrap(),
                before_fees,
                "{trade:?}"
            );
        }
        for side in [order::Side::Sell, order::Side::Buy] {
            for with_policies in [true, false] {
                let count = compared
                    .iter()
                    .filter(|(s, no_policies)| *s == side && *no_policies != with_policies)
                    .count();
                assert!(
                    count > 200,
                    "{side:?} with_policies={with_policies}: {count} trades compared"
                );
            }
        }
    }
}
