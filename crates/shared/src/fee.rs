use {
    alloy::primitives::{U256, U512, ruint::UintTryFrom},
    configs::fee_factor::FeeFactor,
    model::order::OrderKind,
};

/// Everything required to compute the fee amount in sell token
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeeParameters {
    /// The estimated gas units required to execute the quoted trade.
    pub gas_amount: f64,
    /// The estimated gas price at the time of quoting.
    pub gas_price: f64,
    /// The Ether-denominated price of token at the time of quoting.
    ///
    /// The Ether value of `x` sell tokens is `x * sell_token_price`.
    pub sell_token_price: f64,
}

impl Default for FeeParameters {
    fn default() -> Self {
        Self {
            gas_amount: 0.,
            gas_price: 0.,
            // We can't use `derive(Default)` because then this field would have
            // a value of `0.` and it is used in division. The actual value we
            // use here doesn't really matter as long as its non-zero (since the
            // resulting amount in native token or sell token will be 0
            // regardless), but the multiplicative identity seemed like a
            // natural default value to use.
            sell_token_price: 1.,
        }
    }
}

impl FeeParameters {
    pub fn fee(&self) -> U256 {
        self.fee_with_additional_cost(0u64)
    }

    pub fn fee_with_additional_cost(&self, additional_cost: u64) -> U256 {
        let fee_in_eth = (self.gas_amount + additional_cost as f64) * self.gas_price;

        // We want the conversion from f64 to U256 to use ceil because:
        // 1. For final amounts that end up close to 0 atoms we always take a
        //    fee so we are not attackable through low decimal tokens.
        // 2. When validating fees this consistently picks the same amount.
        U256::from((fee_in_eth / self.sell_token_price).ceil())
    }
}

/// Computes the volume fee amount charged against `base_volume` at the given
/// `factor`. High-precision scaling ensures sub-BPS factors don't round to
/// zero.
pub fn compute_volume_fee(base_volume: U256, factor: FeeFactor) -> U256 {
    let scaled_factor = U256::from(factor.to_high_precision());
    let scale = U512::from(FeeFactor::HIGH_PRECISION_SCALE);
    U256::uint_try_from(
        base_volume
            .widening_mul(scaled_factor)
            .checked_div(scale)
            .unwrap_or_default(),
    )
    .expect("FeeFactor is guaranteed to be [0, 1] so result must fit U256")
}

/// Applies a single volume fee to `(sell, buy)` for the given order kind
/// using the amount on the "volume side" of the trade as the fee base:
///
/// - Sell orders: fee is `buy * factor`; the buy amount is reduced.
/// - Buy orders: fee is `sell * factor`; the sell amount is increased.
pub fn apply_volume_fee(sell: U256, buy: U256, kind: OrderKind, factor: FeeFactor) -> (U256, U256) {
    match kind {
        OrderKind::Sell => {
            let fee = compute_volume_fee(buy, factor);
            (sell, buy.saturating_sub(fee))
        }
        OrderKind::Buy => {
            let fee = compute_volume_fee(sell, factor);
            (sell.saturating_add(fee), buy)
        }
    }
}

/// Adjusts quoted `(sell, buy)` for the quote's gas fee (denominated in the
/// sell token), the same way the user-facing quote nets the fee out of the
/// trade (see [`crate::order_quoting::Quote::with_scaled_sell_amount`]):
///
/// - Sell orders keep the full sell and receive proportionally less buy (`buy *
///   (sell - fee) / sell`).
/// - Buy orders keep the exact buy and pay a larger sell (`sell + fee`).
///
/// The fast path must apply this before volume fees so the bid it records
/// matches what the driver settles and what the UI quoted.
pub fn adjust_bid_for_gas_costs(sell: U256, buy: U256, kind: OrderKind, fee: U256) -> (U256, U256) {
    match kind {
        OrderKind::Sell => {
            let net_sell = sell.saturating_sub(fee);
            let adjusted_buy = U256::uint_try_from(
                buy.widening_mul(net_sell)
                    .checked_div(U512::from(sell))
                    .unwrap_or_default(),
            )
            .unwrap_or(U256::MAX);
            (sell, adjusted_buy)
        }
        OrderKind::Buy => (sell.saturating_add(fee), buy),
    }
}

/// The fast-path limit-price check discovered that after applying the
/// expected protocol + partner volume fees the signed order can no
/// longer be settled at the quoted price. Callers convert this into
/// their own error type (a hard placement rejection in the orderbook,
/// a fall-through to the regular auction in the autopilot).
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct FastPathLimitTooTight;

/// Mirrors the on-chain settlement contract's limit-price constraint:
/// `executed_sell * signed_buy <= executed_buy * signed_sell`
///
/// The same check works for sell and buy orders, so `OrderKind` is not
/// needed: any bid that gives the trader a worse price than signed fails
/// it. The multiplication widens to `U512` so wei-scale amounts cannot
/// overflow.
pub fn satisfies_limit_price(
    signed_sell: U256,
    signed_buy: U256,
    executed_sell: U256,
    executed_buy: U256,
) -> bool {
    let lhs: U512 = executed_sell.widening_mul(signed_buy);
    let rhs: U512 = executed_buy.widening_mul(signed_sell);
    lhs <= rhs
}

/// Verifies that the signed sell/buy amounts leave enough room for the
/// compounded volume fees that would be applied at fast-path settlement
/// time.
///
/// `factors` are compounded in order — callers assemble the sequence
/// they expect the autopilot to actually charge (protocol volume fee
/// first, then capped partner fees). The check compares the resulting
/// `(adjusted_sell, adjusted_buy)` against the signed limit via
/// [`satisfies_limit_price`], the same primitive the autopilot's
/// per-solution `filtered_out` classification uses.
pub fn check_fast_path_limit_fits(
    kind: OrderKind,
    signed_sell: U256,
    signed_buy: U256,
    quoted_sell: U256,
    quoted_buy: U256,
    factors: impl IntoIterator<Item = FeeFactor>,
) -> Result<(), FastPathLimitTooTight> {
    let (adjusted_sell, adjusted_buy) = factors
        .into_iter()
        .fold((quoted_sell, quoted_buy), |(sell, buy), factor| {
            apply_volume_fee(sell, buy, kind, factor)
        });

    if satisfies_limit_price(signed_sell, signed_buy, adjusted_sell, adjusted_buy) {
        Ok(())
    } else {
        Err(FastPathLimitTooTight)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn factor(v: f64) -> FeeFactor {
        FeeFactor::try_from(v).unwrap()
    }

    #[test]
    fn apply_volume_fee_sell_order_reduces_buy() {
        let (sell, buy) = apply_volume_fee(
            U256::from(1_000u64),
            U256::from(1_000u64),
            OrderKind::Sell,
            factor(0.01),
        );
        assert_eq!(sell, U256::from(1_000u64));
        assert_eq!(buy, U256::from(990u64));
    }

    #[test]
    fn apply_volume_fee_buy_order_increases_sell() {
        let (sell, buy) = apply_volume_fee(
            U256::from(1_000u64),
            U256::from(1_000u64),
            OrderKind::Buy,
            factor(0.01),
        );
        assert_eq!(sell, U256::from(1_010u64));
        assert_eq!(buy, U256::from(1_000u64));
    }

    #[test]
    fn apply_quote_fee_sell_order_reduces_buy() {
        // Sell keeps its full amount; the buy is scaled by (sell - fee)/sell.
        let (sell, buy) = adjust_bid_for_gas_costs(
            U256::from(1_000u64),
            U256::from(2_000u64),
            OrderKind::Sell,
            U256::from(100u64),
        );
        assert_eq!(sell, U256::from(1_000u64));
        // 2000 * (1000 - 100) / 1000 = 1800
        assert_eq!(buy, U256::from(1_800u64));
    }

    #[test]
    fn apply_quote_fee_buy_order_increases_sell() {
        // Buy keeps its exact amount; the fee is added to the sell.
        let (sell, buy) = adjust_bid_for_gas_costs(
            U256::from(1_000u64),
            U256::from(2_000u64),
            OrderKind::Buy,
            U256::from(100u64),
        );
        assert_eq!(sell, U256::from(1_100u64));
        assert_eq!(buy, U256::from(2_000u64));
    }

    #[test]
    fn apply_quote_fee_zero_fee_is_noop() {
        for kind in [OrderKind::Sell, OrderKind::Buy] {
            let (sell, buy) = adjust_bid_for_gas_costs(
                U256::from(1_000u64),
                U256::from(2_000u64),
                kind,
                U256::ZERO,
            );
            assert_eq!((sell, buy), (U256::from(1_000u64), U256::from(2_000u64)));
        }
    }

    #[test]
    fn compute_volume_fee_sub_bps_uses_high_precision() {
        // 0.3 BPS = 0.00003 must not round to zero.
        let fee = compute_volume_fee(U256::from(1_000_000u64), factor(0.00003));
        assert_eq!(fee, U256::from(30u64));
    }

    #[test]
    fn satisfies_limit_price_output_at_signed_ratio() {
        let signed_sell = U256::from(1_000u64);
        let signed_buy = U256::from(900u64);
        // Full fill at the signed ratio — accepted.
        assert!(satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(1_000u64),
            U256::from(900u64),
        ));
        // 1 wei more output at the same input — better than signed, accepted.
        assert!(satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(1_000u64),
            U256::from(901u64),
        ));
        // 1 wei less output at the same input — trader shortchanged.
        assert!(!satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(1_000u64),
            U256::from(899u64),
        ));
    }

    #[test]
    fn satisfies_limit_price_input_at_signed_ratio() {
        let signed_sell = U256::from(1_100u64);
        let signed_buy = U256::from(1_000u64);
        // Full buy fill at the signed ratio — accepted.
        assert!(satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(1_100u64),
            U256::from(1_000u64),
        ));
        // 1 wei less input for the same output — trader pays less, accepted.
        assert!(satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(1_099u64),
            U256::from(1_000u64),
        ));
        // 1 wei more input for the same output — trader overcharged.
        assert!(!satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(1_101u64),
            U256::from(1_000u64),
        ));
    }

    #[test]
    fn satisfies_limit_price_partial_fill_at_signed_ratio() {
        // A partial fill that keeps the effective price identical to the
        // signed ratio must still be accepted (100 tokens sold for 90 buy
        // matches the same 1000-for-900 rate the trader signed).
        let signed_sell = U256::from(1_000u64);
        let signed_buy = U256::from(900u64);
        assert!(satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(100u64),
            U256::from(90u64),
        ));
        // Partial fill slightly better than the signed ratio — accepted.
        assert!(satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(100u64),
            U256::from(91u64),
        ));
        // Partial fill slightly worse than the signed ratio — rejected.
        assert!(!satisfies_limit_price(
            signed_sell,
            signed_buy,
            U256::from(100u64),
            U256::from(89u64),
        ));
    }
}
