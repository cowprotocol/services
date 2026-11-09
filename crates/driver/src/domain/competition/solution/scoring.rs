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
        trade::CustomClearingPrices,
    },
    crate::domain::competition::{auction, order::FeePolicy, solution::error},
    alloy::primitives::ruint::UintTryFrom,
    chain_types::evm::Evm,
    eth_domain_types as eth,
    number::u256_ext::U256Ext,
};

pub fn compute_score(
    trades: &[Trade],
    native_prices: &auction::Prices,
) -> Result<eth::Ether, Error> {
    trades.iter().map(|trade| trade.score(native_prices)).sum()
}

// Trade represents a single trade in a settlement.
//
// It contains values as expected by the settlement contract. That means that
// clearing prices are adjusted to account for all fees (gas cost and protocol
// fees). Also, executed amount contains the fees for sell order.
#[derive(Debug, Clone)]
pub struct Trade {
    uid: order::Uid,
    /// signed sell token parameters of the order (i.e. limit price)
    signed_sell: eth::Asset,
    /// signed buy token parameters of the order (i.e. limit price)
    signed_buy: eth::Asset,
    side: Side,
    executed: order::TargetAmount,
    /// Price at which the order gets filled. This is based on the solution's
    /// price vector and the necessary adjustements to incorporate fees.
    custom_price: CustomClearingPrices,
    policies: Vec<FeePolicy>,
}

impl Trade {
    pub fn new(
        uid: order::Uid,
        signed_sell: eth::Asset,
        signed_buy: eth::Asset,
        side: Side,
        executed: order::TargetAmount,
        custom_price: CustomClearingPrices,
        policies: Vec<FeePolicy>,
    ) -> Self {
        Self {
            uid,
            signed_sell,
            signed_buy,
            side,
            executed,
            custom_price,
            policies,
        }
    }

    /// Score defined as (surplus + protocol fees) first converted to buy
    /// amounts and then converted to the native token.
    ///
    /// [CIP-38](https://forum.cow.fi/t/cip-38-solver-computed-fees-rank-by-surplus/2061>) as the
    /// base of the score computation.
    /// [Draft CIP](https://forum.cow.fi/t/cip-draft-updating-score-definition-for-buy-orders/2930)
    /// as the latest revision to avoid edge cases for certain buy orders./
    ///
    /// Denominated in NATIVE token
    fn score(&self, native_prices: &auction::Prices) -> Result<eth::Ether, Error> {
        tracing::debug!("Scoring trade {:?}", self);
        let native_price_buy = native_prices
            .get(&self.signed_buy.token)
            .ok_or(Error::MissingPrice(self.signed_buy.token))?;

        let surplus_in_surplus_token = self
            .user_surplus()?
            .0
            .checked_add(self.fees()?.0)
            .ok_or(Error::Math(Math::Overflow))?;

        let score = match self.side {
            // `surplus` of sell orders is already in buy tokens so we simply
            // convert it to ETH
            Side::Sell => native_price_buy.in_eth(eth::TokenAmount(surplus_in_surplus_token)),
            Side::Buy => {
                // `surplus` of buy orders is in sell tokens. We start with
                // following formula: buy_amount / sell_amount
                // == buy_price / sell_price
                //
                // since `surplus` of buy orders is in sell tokens we convert to
                // buy amount via: buy_amount == (buy_price /
                // sell_price) * surplus
                //
                // to avoid loss of precision because we work with integers we
                // first multiply and then divide:
                // buy_amount = surplus * buy_price / sell_price
                let surplus_in_buy_tokens = surplus_in_surplus_token
                    .widening_mul(self.signed_buy.amount.0)
                    .checked_div(alloy::primitives::U512::from(self.signed_sell.amount.0))
                    .ok_or(Error::Math(Math::DivisionByZero))?;
                let surplus_in_buy_tokens = eth::U256::uint_try_from(surplus_in_buy_tokens)
                    .map_err(|_| Error::Math(Math::Overflow))?;

                // Afterwards we convert the buy token surplus to the native
                // token.
                native_price_buy.in_eth(surplus_in_buy_tokens.into())
            }
        };
        Ok(score)
    }

    /// The fill as the generic fee math sees it: the signed limit and the
    /// legs the custom prices settle it at.
    fn fill(&self) -> Result<protocol_fees::Order<Evm>, Math> {
        Ok(protocol_fees::Order {
            uid: chain_types::evm::OrderUid(self.uid.0.0),
            sell_token: self.signed_sell.token.into(),
            buy_token: self.signed_buy.token.into(),
            sell_amount: self.signed_sell.amount.0,
            buy_amount: self.signed_buy.amount.0,
            executed_sell: self.sell_amount()?.0,
            executed_buy: self.buy_amount()?.0,
            side: match self.side {
                Side::Sell => protocol_fees::Side::Sell,
                Side::Buy => protocol_fees::Side::Buy,
            },
        })
    }

    /// The policies as the generic fee math sees them. The score flag is read
    /// by [`Self::fees`], not by the fee math.
    fn policies(&self) -> Vec<protocol_fees::FeePolicy<Evm>> {
        self.policies
            .iter()
            .map(|policy| match policy {
                FeePolicy::Surplus {
                    factor,
                    max_volume_factor,
                } => protocol_fees::FeePolicy::Surplus {
                    factor: *factor,
                    max_volume_factor: *max_volume_factor,
                },
                FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                    quote,
                } => protocol_fees::FeePolicy::PriceImprovement {
                    factor: *factor,
                    max_volume_factor: *max_volume_factor,
                    quote: protocol_fees::Quote {
                        sell_amount: quote.sell.amount.0,
                        buy_amount: quote.buy.amount.0,
                        fee: quote.fee.amount.0,
                        solver: quote.solver,
                    },
                },
                FeePolicy::Volume { factor, .. } => {
                    protocol_fees::FeePolicy::Volume { factor: *factor }
                }
            })
            .collect()
    }

    /// Amount of value the user got above the order's limit price
    /// denominated in the surplus token.
    fn user_surplus(&self) -> Result<eth::SurplusTokenAmount, Error> {
        let surplus = protocol_fees::surplus_over_limit(&self.fill()?).map_err(Math::from)?;
        Ok(eth::SurplusTokenAmount(surplus))
    }

    /// Protocol fees are defined by fee policies attached to the order.
    ///
    /// Every policy is unwound, so each fee is measured at the legs its policy
    /// was applied at. A fee the solver charges for itself (see
    /// [`FeePolicy::contributes_to_score`]) is unwound like any other but left
    /// out of the total.
    fn fees(&self) -> Result<eth::SurplusTokenAmount, Error> {
        let fees = protocol_fees::protocol_fees_by_policy(&self.fill()?, &self.policies())
            .map_err(Math::from)?;
        let scored = self
            .policies
            .iter()
            .zip(fees)
            .filter(|(policy, _)| policy.contributes_to_score())
            .try_fold(eth::U256::ZERO, |total, (_, fee)| total.checked_add(fee))
            .ok_or(Error::Math(Math::Overflow))?;
        Ok(eth::SurplusTokenAmount(scored))
    }

    /// The effective amount that left the user's wallet including all fees.
    ///
    /// Note how the `executed` amount is used to build actual traded amounts.
    fn sell_amount(&self) -> Result<eth::TokenAmount, Math> {
        Ok(match self.side {
            Side::Sell => self.executed.0,
            Side::Buy => self
                .executed
                .0
                .checked_mul_ratio(&self.custom_price.buy, &self.custom_price.sell)?,
        }
        .into())
    }

    /// The effective amount the user received after all fees.
    ///
    /// Note how the `executed` amount is used to build actual traded amounts.
    ///
    /// Settlement contract uses `ceil` division for buy amount calculation.
    fn buy_amount(&self) -> Result<eth::TokenAmount, Math> {
        Ok(match self.side {
            Side::Sell => self
                .executed
                .0
                .checked_mul_ratio_ceil(&self.custom_price.sell, &self.custom_price.buy)?,
            Side::Buy => self.executed.0,
        }
        .into())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
    #[error("scoring: failed to calculate custom price for the applied fee policy {0:?}")]
    Scoring(#[source] error::Scoring),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::competition::{
            PriceLimits,
            solution::fee::{self, adjust_quote_to_order_limits},
        },
        alloy::primitives::{Address, U256, address},
        auction::Price,
        rand::{Rng, SeedableRng, rngs::StdRng},
        std::collections::HashMap,
    };

    /// Tests that the new score computation limits the score of certain
    /// buy orders to a reasonable amount.
    /// NOTE(historical): Data is based on this
    /// [auction](https://api.cow.fi/base/api/v1/solver_competition/by_tx_hash/0xe3ef02493255f17c0abd2ff88c34682d35f0de4f4875a4653104e3453473d8d9).
    #[test]
    fn score_problematic_buy_order() {
        const WETH: Address = address!("4200000000000000000000000000000000000006");
        const BNKR: Address = address!("22af33fe49fd1fa80c7149773dde5890d3c76f3b");

        // Buy order which results in an unreasonably high score
        // using the original scoring mechanism.
        let trade = Trade {
            uid: order::Uid(Default::default()),
            signed_sell: eth::Asset {
                token: WETH.into(),
                amount: 9865986634773384514560000000000000u128.into(),
            },
            signed_buy: eth::Asset {
                token: BNKR.into(),
                amount: 4025333872768468868566822740u128.into(),
            },
            side: Side::Buy,
            executed: order::TargetAmount(U256::from(8050667745u128)),
            custom_price: CustomClearingPrices {
                sell: U256::from(874045870u128),
                buy: U256::from(8050667745u128),
            },
            policies: vec![],
        };

        let native_prices: HashMap<_, _> = [
            (
                WETH.into(),
                Price(eth::Ether(U256::from(1000000000000000000u128))),
            ),
            (BNKR.into(), Price(eth::Ether(U256::from(113181296327u128)))),
        ]
        .into_iter()
        .collect();

        let score = trade.score(&native_prices).unwrap();
        assert_eq!(score.0, U256::from(911));
    }

    /// A fee the solver keeps for itself does not count towards the score, but
    /// it still has to be unwound so that the protocol fee applied before it is
    /// measured against the output it actually took its cut from.
    ///
    /// Raw output 1000, of which a 10% protocol fee takes 100, leaving 900, and
    /// a 20% solver fee takes 180, leaving the user 720. The protocol's 100 is
    /// what the score may count; reconstructing it from the 720 the user ended
    /// up with would yield 80.
    #[test]
    fn solver_fee_is_unwound_but_not_scored() {
        const SELL: Address = address!("0000000000000000000000000000000000000001");
        const BUY: Address = address!("0000000000000000000000000000000000000002");

        let trade = |policies| Trade {
            uid: order::Uid(Default::default()),
            signed_sell: eth::Asset {
                token: SELL.into(),
                amount: U256::from(1000u128).into(),
            },
            // Signed at what the user ends up with, so all that is left in the
            // score is the fees.
            signed_buy: eth::Asset {
                token: BUY.into(),
                amount: U256::from(720u128).into(),
            },
            side: Side::Sell,
            executed: order::TargetAmount(U256::from(1000u128)),
            custom_price: CustomClearingPrices {
                sell: U256::from(720u128),
                buy: U256::from(1000u128),
            },
            policies,
        };
        let protocol_fee = FeePolicy::Volume {
            factor: 0.1,
            contributes_to_score: true,
        };
        let solver_fee = FeePolicy::Volume {
            factor: 0.2,
            contributes_to_score: false,
        };

        let native_prices: HashMap<_, _> = [
            (
                SELL.into(),
                Price(eth::Ether(U256::from(1000000000000000000u128))),
            ),
            (
                BUY.into(),
                Price(eth::Ether(U256::from(1000000000000000000u128))),
            ),
        ]
        .into_iter()
        .collect();

        let scored = trade(vec![protocol_fee.clone(), solver_fee])
            .score(&native_prices)
            .unwrap();
        assert_eq!(scored.0, U256::from(100u128));

        // Dropping the solver fee from the list instead of unwinding it is what
        // undercounts the protocol fee.
        let dropped = trade(vec![protocol_fee]).score(&native_prices).unwrap();
        assert_eq!(dropped.0, U256::from(80u128));
    }

    /// The driver's own scoring formulas, kept as the oracle for the equality
    /// test below. One difference: legacy also rebuilt the prices after the
    /// first policy, where nothing reads them, and could fail there. The
    /// oracle skips that rebuild so the errors compare too.
    mod legacy {
        use super::*;

        fn with_custom_price(trade: &Trade, custom_price: CustomClearingPrices) -> Trade {
            Trade {
                custom_price,
                ..trade.clone()
            }
        }

        fn surplus_over(
            trade: &Trade,
            price_limits: PriceLimits,
        ) -> Result<eth::SurplusTokenAmount, Math> {
            match trade.side {
                Side::Buy => {
                    let limit_sell = price_limits
                        .sell
                        .0
                        .checked_mul(trade.executed.into())
                        .ok_or(Math::Overflow)?
                        .checked_div(price_limits.buy.0)
                        .ok_or(Math::DivisionByZero)?;
                    let sold = trade
                        .executed
                        .0
                        .checked_mul(trade.custom_price.buy)
                        .ok_or(Math::Overflow)?
                        .checked_div(trade.custom_price.sell)
                        .ok_or(Math::DivisionByZero)?;
                    limit_sell.checked_sub(sold).ok_or(Math::Negative)
                }
                Side::Sell => {
                    let limit_buy = trade
                        .executed
                        .0
                        .checked_mul(price_limits.buy.0)
                        .ok_or(Math::Overflow)?
                        .checked_ceil_div(&price_limits.sell.0)
                        .ok_or(Math::DivisionByZero)?;
                    let bought = trade
                        .executed
                        .0
                        .checked_mul(trade.custom_price.sell)
                        .ok_or(Math::Overflow)?
                        .checked_ceil_div(&trade.custom_price.buy)
                        .ok_or(Math::DivisionByZero)?;
                    bought.checked_sub(limit_buy).ok_or(Math::Negative)
                }
            }
            .map(eth::SurplusTokenAmount)
        }

        pub fn fees(trade: &Trade) -> Result<eth::SurplusTokenAmount, Error> {
            let mut current_trade = trade.clone();
            let mut unwound = eth::SurplusTokenAmount::default();
            let mut scored = eth::SurplusTokenAmount::default();
            for (i, protocol_fee) in trade.policies.iter().enumerate().rev() {
                let fee = self::protocol_fee(&current_trade, protocol_fee)?;
                unwound = unwound
                    .0
                    .checked_add(fee.0)
                    .ok_or(Error::Math(Math::Overflow))?
                    .into();
                if protocol_fee.contributes_to_score() {
                    scored = scored
                        .0
                        .checked_add(fee.0)
                        .ok_or(Error::Math(Math::Overflow))?
                        .into();
                }
                if i != 0 {
                    current_trade =
                        with_custom_price(trade, calculate_custom_prices(trade, unwound)?);
                }
            }
            Ok(scored)
        }

        fn sell_amount(trade: &Trade) -> Result<eth::TokenAmount, Math> {
            Ok(match trade.side {
                Side::Sell => trade.executed.0,
                Side::Buy => trade
                    .executed
                    .0
                    .checked_mul(trade.custom_price.buy)
                    .ok_or(Math::Overflow)?
                    .checked_div(trade.custom_price.sell)
                    .ok_or(Math::DivisionByZero)?,
            }
            .into())
        }

        fn buy_amount(trade: &Trade) -> Result<eth::TokenAmount, Math> {
            Ok(match trade.side {
                Side::Sell => trade
                    .executed
                    .0
                    .checked_mul(trade.custom_price.sell)
                    .ok_or(Math::Overflow)?
                    .checked_ceil_div(&trade.custom_price.buy)
                    .ok_or(Math::DivisionByZero)?,
                Side::Buy => trade.executed.0,
            }
            .into())
        }

        fn calculate_custom_prices(
            trade: &Trade,
            protocol_fee: eth::SurplusTokenAmount,
        ) -> Result<CustomClearingPrices, Math> {
            Ok(CustomClearingPrices {
                sell: match trade.side {
                    Side::Sell => buy_amount(trade)?
                        .0
                        .checked_add(protocol_fee.0)
                        .ok_or(Math::Overflow)?,
                    Side::Buy => buy_amount(trade)?.0,
                },
                buy: match trade.side {
                    Side::Sell => sell_amount(trade)?.0,
                    Side::Buy => sell_amount(trade)?
                        .0
                        .checked_sub(protocol_fee.0)
                        .ok_or(Math::Negative)?,
                },
            })
        }

        fn protocol_fee(
            trade: &Trade,
            fee_policy: &FeePolicy,
        ) -> Result<eth::SurplusTokenAmount, Error> {
            let amount = match fee_policy {
                FeePolicy::Surplus {
                    factor,
                    max_volume_factor,
                } => {
                    let surplus = user_surplus(trade)?;
                    std::cmp::min(
                        fee(surplus, *factor)?,
                        volume_fee(trade, *max_volume_factor)?,
                    )
                }
                FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                    quote,
                } => {
                    let price_improvement = price_improvement(trade, quote)?;
                    std::cmp::min(
                        fee(price_improvement, *factor)?,
                        volume_fee(trade, *max_volume_factor)?,
                    )
                }
                FeePolicy::Volume { factor, .. } => volume_fee(trade, *factor)?,
            };
            Ok(eth::SurplusTokenAmount(amount.0))
        }

        fn price_improvement(
            trade: &Trade,
            quote: &order::Quote,
        ) -> Result<eth::SurplusTokenAmount, Error> {
            let quote = adjust_quote_to_order_limits(
                fee::Order {
                    sell_amount: trade.signed_sell.amount.0,
                    buy_amount: trade.signed_buy.amount.0,
                    side: trade.side,
                },
                fee::Quote {
                    sell_amount: quote.sell.amount.0,
                    buy_amount: quote.buy.amount.0,
                    fee_amount: quote.fee.amount.0,
                },
            )?;
            let surplus = surplus_over(trade, quote);
            if let Err(Math::Negative) = surplus {
                return Ok(eth::SurplusTokenAmount(eth::U256::ZERO));
            }
            Ok(surplus?)
        }

        pub fn user_surplus(trade: &Trade) -> Result<eth::SurplusTokenAmount, Error> {
            let limit_price = PriceLimits {
                sell: trade.signed_sell.amount,
                buy: trade.signed_buy.amount,
            };
            Ok(surplus_over(trade, limit_price)?)
        }

        fn fee(
            surplus: eth::SurplusTokenAmount,
            factor: f64,
        ) -> Result<eth::SurplusTokenAmount, Error> {
            let multiplied = surplus
                .0
                .checked_mul_f64(factor / (1.0 - factor))
                .ok_or(Error::Math(Math::Overflow))?;
            Ok(multiplied.into())
        }

        fn volume_fee(trade: &Trade, factor: f64) -> Result<eth::SurplusTokenAmount, Error> {
            let executed_in_surplus_token = match trade.side {
                Side::Buy => eth::SurplusTokenAmount(sell_amount(trade)?.0),
                Side::Sell => eth::SurplusTokenAmount(buy_amount(trade)?.0),
            };
            let factor = match trade.side {
                Side::Sell => factor / (1.0 - factor),
                Side::Buy => factor / (1.0 + factor),
            };
            let multiplied = executed_in_surplus_token
                .0
                .checked_mul_f64(factor)
                .ok_or(Error::Math(Math::Overflow))?;
            Ok(multiplied.into())
        }
    }

    fn asset(amount: u64) -> eth::Asset {
        eth::Asset {
            token: Address::repeat_byte(1).into(),
            amount: U256::from(amount).into(),
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
                let sell = rng.random_range(1..=1u64 << 48);
                FeePolicy::PriceImprovement {
                    factor,
                    max_volume_factor,
                    quote: order::Quote {
                        sell: asset(sell),
                        buy: asset(rng.random_range(1..=1u64 << 48)),
                        fee: asset(rng.random_range(0..=sell / 2)),
                        solver: Address::repeat_byte(9),
                    },
                }
            }
        }
    }

    /// A trade on either side at a random price vector: the legs scaled by a
    /// random factor, so the custom prices are not the legs themselves.
    fn random_trade(rng: &mut StdRng) -> Trade {
        let side = if rng.random_bool(0.5) {
            Side::Sell
        } else {
            Side::Buy
        };
        let sell_amount = rng.random_range(1..=1u64 << 48);
        let buy_amount = rng.random_range(1..=1u64 << 48);
        let surplus = rng.random_range(0..=1u64 << 47);
        let (executed_sell, executed_buy) = match side {
            Side::Sell => {
                let executed_sell = rng.random_range(1..=sell_amount);
                let at_limit = (u128::from(executed_sell) * u128::from(buy_amount))
                    .div_ceil(u128::from(sell_amount)) as u64;
                (executed_sell, at_limit + surplus)
            }
            Side::Buy => {
                let executed_buy = rng.random_range(1..=buy_amount);
                let at_limit = (u128::from(executed_buy) * u128::from(sell_amount)
                    / u128::from(buy_amount)) as u64;
                (at_limit.saturating_sub(surplus).max(1), executed_buy)
            }
        };
        let scale = U256::from(rng.random_range(1..=1_000u64));
        Trade {
            uid: order::Uid(Default::default()),
            signed_sell: asset(sell_amount),
            signed_buy: asset(buy_amount),
            side,
            executed: order::TargetAmount(U256::from(match side {
                Side::Sell => executed_sell,
                Side::Buy => executed_buy,
            })),
            custom_price: CustomClearingPrices {
                sell: U256::from(executed_buy) * scale,
                buy: U256::from(executed_sell) * scale,
            },
            policies: (0..rng.random_range(0..=3))
                .map(|_| random_policy(rng))
                .collect(),
        }
    }

    /// The generic math gives the legacy formulas' numbers and refuses the
    /// trades they refuse.
    #[test]
    fn fees_and_surplus_match_the_legacy_formulas() {
        let mut rng = StdRng::seed_from_u64(0x5eed);
        let mut compared = Vec::new();
        for _ in 0..5_000 {
            let trade = random_trade(&mut rng);
            let expected =
                legacy::fees(&trade).and_then(|fees| Ok((fees, legacy::user_surplus(&trade)?)));
            let actual = trade
                .fees()
                .and_then(|fees| Ok((fees, trade.user_surplus()?)));
            match (expected, actual) {
                (Ok(expected), Ok(actual)) => {
                    assert_eq!(actual, expected, "{trade:?}");
                    compared.push((trade.side, trade.policies.is_empty()));
                }
                (Ok(_), Err(err)) => panic!("refused {err:?} {trade:?}"),
                (Err(err), Ok(_)) => panic!("legacy refused {err:?} {trade:?}"),
                (Err(_), Err(_)) => {}
            }
        }
        for side in [Side::Sell, Side::Buy] {
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
