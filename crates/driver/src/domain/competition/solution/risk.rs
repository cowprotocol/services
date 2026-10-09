//! Risk-adjusted bidding.
//!
//! A winning solution that does not settle in time costs the solver a penalty
//! of at most the sum of the penalty caps of its orders (CIP-87). Given the
//! probability `p` that the solution settles, its full score `S` and its
//! penalty cap `c_l`, the score at which winning breaks even in expectation is
//!
//! `s = max(p * S, S - (1 - p) / p * c_l)`
//!
//! Bidding this score is a dominant strategy as long as the reward cap does
//! not bind, see the [optimal bidding](https://docs.cow.fi/cow-protocol/tutorials/solvers/optimal-bidding)
//! docs. Since the score is derived from the traded amounts, the driver bids
//! it by withholding the margin `S - s` from the users, spread across the
//! solution's trades in proportion to their surplus. The solver keeps it.

use {
    super::{
        error::Math,
        trade::{ClearingPrices, Fulfillment},
    },
    crate::domain::competition::{auction, order::Side},
    eth_domain_types as eth,
    number::u256_ext::U256Ext,
};

/// 10^18, the fixed point scale of [`SuccessProbability`].
const WAD: eth::U256 = eth::U256::from_limbs([1_000_000_000_000_000_000, 0, 0, 0]);

/// Probability that a winning solution settles before the deadline, as a
/// fixed point number scaled by 10^18 to keep the margin math exact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SuccessProbability(eth::U256);

impl SuccessProbability {
    pub fn new(value: f64) -> Result<Self, Error> {
        if !(value > 0.0 && value <= 1.0) {
            return Err(Error::InvalidSuccessProbability(value));
        }
        // In range, so the scaled value fits and is at most `WAD`.
        let scaled = eth::U256::from((value * 1e18).round() as u128).min(WAD);
        if scaled.is_zero() {
            return Err(Error::InvalidSuccessProbability(value));
        }
        Ok(Self(scaled))
    }
}

/// Computes the margin (in surplus token) to withhold from each fulfillment
/// so the solution bids its risk-adjusted score. `fulfillments` must contain
/// all trades contributing to the score, with protocol fees not yet applied.
pub fn margins(
    fulfillments: &[(&Fulfillment, ClearingPrices)],
    success_probability: SuccessProbability,
    native_prices: &auction::Prices,
) -> Result<Vec<eth::TokenAmount>, Error> {
    let trades = fulfillments
        .iter()
        .map(|(fulfillment, prices)| TradeValue::new(fulfillment, *prices, native_prices))
        .collect::<Result<Vec<_>, _>>()?;
    compute(&trades, success_probability)
}

/// What a single trade contributes to the solution's score and penalty cap.
#[derive(Debug)]
struct TradeValue {
    /// Surplus (including protocol fees) in surplus token.
    surplus: eth::U256,
    /// `surplus` converted to the native token.
    surplus_native: eth::U256,
    /// Penalty cap of the order, scaled to the executed fraction.
    penalty_cap: Option<eth::U256>,
}

impl TradeValue {
    fn new(
        fulfillment: &Fulfillment,
        prices: ClearingPrices,
        native_prices: &auction::Prices,
    ) -> Result<Self, Error> {
        let order = fulfillment.order();
        let surplus = fulfillment
            .surplus_over_reference_price(order.sell.amount.0, order.buy.amount.0, prices)?
            .0;

        // Same conversion as the score: surplus of buy orders is converted
        // into the buy token at the limit price first.
        let native_price_buy = native_prices
            .get(&order.buy.token)
            .ok_or(Error::MissingPrice(order.buy.token))?;
        let surplus_in_buy_token = match order.side {
            Side::Sell => surplus,
            Side::Buy => surplus
                .checked_mul_ratio(&order.buy.amount.0, &order.sell.amount.0)
                .map_err(Math::from)?,
        };
        let surplus_native = native_price_buy.in_eth(surplus_in_buy_token.into()).0;

        // The penalty cap is computed for the full order, so it has to be
        // scaled to the fraction this trade executes.
        let executed = match order.side {
            Side::Sell => fulfillment
                .executed()
                .0
                .checked_add(fulfillment.fee().0)
                .ok_or(Math::Overflow)?,
            Side::Buy => fulfillment.executed().0,
        };
        let penalty_cap = order
            .penalty_cap_native
            .map(|cap| cap.0.checked_mul_ratio(&executed, &order.target().0))
            .transpose()
            .map_err(Math::from)?;

        Ok(Self {
            surplus,
            surplus_native,
            penalty_cap,
        })
    }
}

/// Splits the margin `S - s = min((1 - p) * S, (1 - p) / p * c_l)` across the
/// trades in proportion to their surplus. Without a penalty cap for every
/// order the penalty is assumed to be uncapped, i.e. the margin is
/// `(1 - p) * S`.
fn compute(
    trades: &[TradeValue],
    success_probability: SuccessProbability,
) -> Result<Vec<eth::TokenAmount>, Error> {
    let p = success_probability.0;
    let failure = WAD - p;
    let surplus_native = trades.iter().try_fold(eth::U256::ZERO, |acc, trade| {
        acc.checked_add(trade.surplus_native).ok_or(Math::Overflow)
    })?;
    let penalty_cap = trades
        .iter()
        .try_fold(Some(eth::U256::ZERO), |acc, trade| {
            match (acc, trade.penalty_cap) {
                (Some(acc), Some(cap)) => acc.checked_add(cap).ok_or(Math::Overflow).map(Some),
                _ => Ok(None),
            }
        })?;

    trades
        .iter()
        .map(|trade| {
            if surplus_native.is_zero() {
                return Ok(eth::U256::ZERO);
            }
            let uncapped = trade
                .surplus
                .checked_mul_ratio(&failure, &WAD)
                .map_err(Math::from)?;
            let Some(penalty_cap) = penalty_cap else {
                return Ok(uncapped);
            };
            // This trade's share of `c_l`, in its surplus token.
            let capped = trade
                .surplus
                .checked_mul_ratio(&penalty_cap, &surplus_native)
                .map_err(Math::from)?
                .checked_mul_ratio(&failure, &p)
                .map_err(Math::from)?;
            Ok(uncapped.min(capped))
        })
        .map(|margin| margin.map(eth::TokenAmount))
        .collect()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("success probability {0} is not in (0, 1]")]
    InvalidSuccessProbability(f64),
    #[error("missing native price for token {0:?}")]
    MissingPrice(eth::TokenAddress),
    #[error(transparent)]
    Math(#[from] Math),
    #[error(transparent)]
    Trade(#[from] super::error::Trade),
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            domain::competition::{
                self,
                order::{
                    BuyTokenBalance,
                    FeePolicy,
                    OrderData,
                    Partial,
                    SellTokenBalance,
                    Signature,
                    signature,
                },
                solution::scoring,
            },
            util,
        },
        alloy::primitives::{U256, address},
        number::{testing::ApproxEq, units::EthUnit},
        std::{collections::HashMap, sync::Arc},
    };

    fn trade(surplus: u64, penalty_cap: Option<u64>) -> TradeValue {
        TradeValue {
            surplus: U256::from(surplus),
            surplus_native: U256::from(surplus),
            penalty_cap: penalty_cap.map(U256::from),
        }
    }

    fn margins(trades: &[TradeValue], p: f64) -> Vec<u64> {
        compute(trades, SuccessProbability::new(p).unwrap())
            .unwrap()
            .into_iter()
            .map(|margin| margin.0.to())
            .collect()
    }

    #[test]
    fn rejects_invalid_probability() {
        assert!(SuccessProbability::new(0.0).is_err());
        assert!(SuccessProbability::new(1.01).is_err());
        assert!(SuccessProbability::new(f64::NAN).is_err());
        assert!(SuccessProbability::new(1.0).is_ok());
    }

    #[test]
    fn certain_settlement_bids_full_score() {
        assert_eq!(margins(&[trade(1000, Some(20))], 1.0), vec![0]);
    }

    /// S = 1000, p = 0.8, c_l = 20: s = max(800, 1000 - 0.25 * 20) = 995.
    #[test]
    fn penalty_cap_binds() {
        assert_eq!(margins(&[trade(1000, Some(20))], 0.8), vec![5]);
    }

    /// S = 10, p = 0.8, c_l = 20: s = max(8, 10 - 5) = 8.
    #[test]
    fn penalty_cap_does_not_bind() {
        assert_eq!(margins(&[trade(10, Some(20))], 0.8), vec![2]);
    }

    #[test]
    fn missing_penalty_cap_is_uncapped() {
        assert_eq!(margins(&[trade(1000, None)], 0.8), vec![200]);
        assert_eq!(
            margins(&[trade(1000, Some(20)), trade(1000, None)], 0.8),
            vec![200, 200]
        );
    }

    /// S = 3000, c_l = 30: the margin of 7.5 is split 1:2.
    #[test]
    fn margin_is_computed_per_solution_and_split_by_surplus() {
        assert_eq!(
            margins(&[trade(1000, Some(10)), trade(2000, Some(20))], 0.8),
            vec![2, 5]
        );
    }

    #[test]
    fn no_surplus_no_margin() {
        assert_eq!(margins(&[trade(0, Some(20))], 0.5), vec![0]);
        assert_eq!(margins(&[], 0.5), Vec::<u64>::new());
    }

    const SELL: eth::Address = address!("0000000000000000000000000000000000000001");
    const BUY: eth::Address = address!("0000000000000000000000000000000000000002");

    fn fulfillment(
        side: Side,
        sell: U256,
        buy: U256,
        executed: U256,
        partial: Partial,
        penalty_cap: U256,
        protocol_fees: Vec<FeePolicy>,
    ) -> Fulfillment {
        let order = competition::Order {
            data: Arc::new(OrderData {
                uid: Default::default(),
                receiver: Default::default(),
                created: util::Timestamp(100),
                valid_to: util::Timestamp(u32::MAX),
                sell: eth::Asset {
                    token: SELL.into(),
                    amount: sell.into(),
                },
                buy: eth::Asset {
                    token: BUY.into(),
                    amount: buy.into(),
                },
                side,
                pre_interactions: Default::default(),
                post_interactions: Default::default(),
                sell_token_balance: SellTokenBalance::Erc20,
                buy_token_balance: BuyTokenBalance::Erc20,
                signature: Signature {
                    scheme: signature::Scheme::PreSign,
                    data: Default::default(),
                    signer: Default::default(),
                },
                protocol_fees,
                quote: Default::default(),
                penalty_cap_native: Some(eth::Ether(penalty_cap)),
            }),
            app_data: Default::default(),
            partial,
        };
        Fulfillment::new(order, executed.into(), Default::default()).unwrap()
    }

    fn native_prices() -> auction::Prices {
        [SELL, BUY]
            .into_iter()
            .map(|token| (token.into(), auction::Price(eth::Ether(1u64.eth()))))
            .collect::<HashMap<_, _>>()
    }

    /// Bids the solution the way the driver does (margin, then protocol fees)
    /// and scores it like the autopilot does (unwinding the protocol fees).
    fn score(fulfillment: &Fulfillment, p: f64) -> (U256, U256) {
        let prices = ClearingPrices {
            sell: U256::ONE,
            buy: U256::ONE,
        };
        let native_prices = native_prices();
        let margin = super::margins(
            &[(fulfillment, prices)],
            SuccessProbability::new(p).unwrap(),
            &native_prices,
        )
        .unwrap()[0];
        let bid = fulfillment
            .with_risk_margin(prices, margin)
            .unwrap()
            .with_protocol_fees(prices)
            .unwrap();
        let order = bid.order();
        let executed = match order.side {
            Side::Sell => (bid.executed().0 + bid.fee().0).into(),
            Side::Buy => bid.executed(),
        };
        let trade = scoring::Trade::new(
            order.sell,
            order.buy,
            order.side,
            executed,
            bid.custom_prices(&prices).unwrap(),
            order.protocol_fees.clone(),
        );
        let score = scoring::compute_score(&[trade], &native_prices).unwrap();
        (score.0, margin.0)
    }

    fn half_eth() -> U256 {
        1u64.eth() / U256::from(2u64)
    }

    fn surplus_protocol_fee() -> Vec<FeePolicy> {
        vec![FeePolicy::Surplus {
            factor: 0.5,
            max_volume_factor: 0.1,
        }]
    }

    /// Selling 1000 A for at least 900 B at 1:1 yields S = 100. With p = 0.8
    /// and c_l = 2 the bid is max(80, 100 - 0.25 * 2) = 99.5, regardless of
    /// the protocol fee taken afterwards.
    #[test]
    fn sell_order_bids_risk_adjusted_score() {
        let trade = fulfillment(
            Side::Sell,
            1000u64.eth(),
            900u64.eth(),
            1000u64.eth(),
            Partial::No,
            2u64.eth(),
            surplus_protocol_fee(),
        );
        let (score, margin) = score(&trade, 0.8);
        assert_eq!(margin, half_eth());
        assert!(score.is_approx_eq(&(100u64.eth() - half_eth()), None));
    }

    /// Buying 1000 B for at most 1100 A at 1:1 yields 100 A of surplus, worth
    /// S = 100 * 1000 / 1100 B. With c_l = 2 the score drops by 0.5.
    #[test]
    fn buy_order_bids_risk_adjusted_score() {
        let trade = fulfillment(
            Side::Buy,
            1100u64.eth(),
            1000u64.eth(),
            1000u64.eth(),
            Partial::No,
            2u64.eth(),
            surplus_protocol_fee(),
        );
        let full = U256::from(100u64).saturating_mul(1000u64.eth()) / U256::from(1100u64);
        let (score, _) = score(&trade, 0.8);
        assert!(score.is_approx_eq(&(full - half_eth()), None));
    }

    /// Executing half of a partially fillable order halves its penalty cap.
    #[test]
    fn partial_fill_scales_penalty_cap() {
        let trade = fulfillment(
            Side::Sell,
            1000u64.eth(),
            900u64.eth(),
            500u64.eth(),
            Partial::Yes {
                available: 1000u64.eth().into(),
            },
            2u64.eth(),
            vec![],
        );
        let value = TradeValue::new(
            &trade,
            ClearingPrices {
                sell: U256::ONE,
                buy: U256::ONE,
            },
            &native_prices(),
        )
        .unwrap();
        assert_eq!(value.penalty_cap, Some(1u64.eth()));
        assert_eq!(value.surplus, 50u64.eth());
    }
}
