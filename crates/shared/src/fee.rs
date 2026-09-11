use {
    crate::{arguments::TokenBucketFeeOverride, order_validation::is_same_buy_and_sell_token},
    alloy::primitives::{Address, U256, U512, ruint::UintTryFrom},
    configs::fee_factor::FeeFactor,
    model::order::{BUY_ETH_ADDRESS, OrderKind},
    rust_decimal::Decimal,
};

/// Number of basis points that make up 100%. Used across bps ↔ decimal
/// conversions in this module.
const MAX_BPS: u32 = 10_000;

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

pub struct VolumeFeePolicy {
    bucket_overrides: Vec<TokenBucketFeeOverride>,
    default_factor: Option<FeeFactor>,
    enable_sell_equals_buy_volume_fee: bool,
    native_token: Address,
}

impl VolumeFeePolicy {
    pub fn new(
        bucket_overrides: Vec<TokenBucketFeeOverride>,
        default_factor: Option<FeeFactor>,
        enable_sell_equals_buy_volume_fee: bool,
        native_token: Address,
    ) -> Self {
        // Treat native-ETH trades as wrapped native token trades
        let bucket_overrides = bucket_overrides
            .into_iter()
            .map(|mut bucket| {
                if bucket.tokens.contains(&BUY_ETH_ADDRESS) {
                    bucket.tokens.insert(native_token);
                }
                bucket
            })
            .collect();
        Self {
            bucket_overrides,
            default_factor,
            enable_sell_equals_buy_volume_fee,
            native_token,
        }
    }

    /// Determines the applicable volume fee factor for a token pair,
    /// considering same-token trade configuration, token bucket overrides
    /// and default fee factor.
    ///
    /// `fee_factor_override` can be used to provide an ad-hoc default factor
    /// which is useful in autopilot where the factor is not known upfront.
    pub fn get_applicable_volume_fee_factor(
        &self,
        buy_token: Address,
        sell_token: Address,
        fee_factor: Option<FeeFactor>,
    ) -> Option<FeeFactor> {
        // Skip the volume fee for same-token trades (treating a native-ETH buy
        // as the wrapped token, so e.g. WETH->ETH is a no-op) unless
        // the flag is set.
        if !self.enable_sell_equals_buy_volume_fee
            && is_same_buy_and_sell_token(sell_token, buy_token, self.native_token)
        {
            return None;
        }

        // Treat native-ETH buys as the wrapped native token so buckets don't
        // need to list the ETH marker address explicitly.
        let buy_token = if buy_token == BUY_ETH_ADDRESS {
            self.native_token
        } else {
            buy_token
        };

        // Check for token bucket overrides first (both tokens must be in the
        // same bucket)
        for fee_override in &self.bucket_overrides {
            if fee_override.tokens.contains(&buy_token) && fee_override.tokens.contains(&sell_token)
            {
                return Some(fee_override.factor);
            }
        }

        // Fall back to default factor either from argument or configured
        // default
        fee_factor.or(self.default_factor)
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
    .unwrap_or(U256::MAX)
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

/// Applies the partner-fee compounding cap to a single requested fee
/// factor and updates the running accumulator. Both the autopilot's
/// `ProtocolFees::apply` and the orderbook's fast-path limit-price check
/// route their per-fee cap decisions through this helper — the tricky
/// multiplicative-cap math lives here in exactly one place.
///
/// Fees compound as `(1 + f_1)(1 + f_2)…` so the accumulator tracks the
/// combined "extra" already committed and the remaining headroom is
/// `(1 + cap)/(1 + accumulated) - 1`. Returns the effective factor for
/// this fee (already clamped into `[0, remaining_factor]`) and advances
/// the accumulator by the additive portion that was allowed through.
pub fn capped_fee_factor(value: Decimal, cap: Decimal, accumulated: &mut Decimal) -> FeeFactor {
    let remaining_factor = (Decimal::ONE + cap) / (Decimal::ONE + *accumulated) - Decimal::ONE;
    *accumulated += value.min(cap - *accumulated);
    FeeFactor::new(f64::try_from(value.max(Decimal::ZERO).min(remaining_factor)).unwrap())
}

/// Extracts the volume-type partner fee factors from parsed app-data,
/// enforcing the compounding cap via [`capped_fee_factor`]. Non-volume
/// partner policies are skipped — they don't eat into the user-facing
/// volume budget — so this yields exactly the subset of factors the
/// orderbook needs to size the fast-path limit-price check against.
pub fn capped_partner_volume_factors(
    parsed_app_data: &app_data::ProtocolAppData,
    max_partner_fee: FeeFactor,
) -> Vec<FeeFactor> {
    let Ok(cap) = Decimal::try_from(max_partner_fee.get()) else {
        return vec![];
    };

    let mut accumulated = Decimal::ZERO;
    let mut factors = Vec::new();
    for partner_fee in parsed_app_data.partner_fee.iter() {
        let app_data::FeePolicy::Volume { bps } = partner_fee.policy else {
            continue;
        };
        let requested = Decimal::from(bps) / Decimal::from(MAX_BPS);
        let factor = capped_fee_factor(requested, cap, &mut accumulated);
        if factor.get() > 0.0 {
            factors.push(factor);
        }
    }
    factors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_fee_bucket_override() {
        let usdc = testlib::tokens::USDC;
        let dai = testlib::tokens::DAI;
        let usdt = testlib::tokens::USDT;
        let weth = testlib::tokens::WETH;

        let bucket_pair_override = TokenBucketFeeOverride {
            tokens: [usdc, dai].into_iter().collect(),
            factor: FeeFactor::try_from(0.0005).unwrap(), // 0.05%
        };
        let bucket_group_override = TokenBucketFeeOverride {
            tokens: [usdc, dai, usdt].into_iter().collect(),
            factor: FeeFactor::try_from(0.0).unwrap(), // 0%
        };

        let default_fee = FeeFactor::try_from(0.001).unwrap(); // 0.1%
        let volume_fee_policy = VolumeFeePolicy::new(
            vec![bucket_pair_override, bucket_group_override],
            Some(default_fee),
            false,
            weth,
        );

        // USDC-DAI (matches both buckets) - pair bucket takes precedence
        let override_ = volume_fee_policy.get_applicable_volume_fee_factor(usdc, dai, None);
        assert_eq!(override_, Some(FeeFactor::try_from(0.0005).unwrap()));

        // DAI-USDT (only in 3-token bucket) - should have override
        let override_ = volume_fee_policy.get_applicable_volume_fee_factor(dai, usdt, None);
        assert_eq!(override_, Some(FeeFactor::try_from(0.0).unwrap()));

        // WETH-DAI (only one in bucket) - should fall back to default fee
        let override_ = volume_fee_policy.get_applicable_volume_fee_factor(weth, dai, None);
        assert_eq!(override_, Some(default_fee));
    }

    #[test]
    fn test_eth_buy_matches_wrapped_native_token_bucket() {
        let weth = testlib::tokens::WETH;
        let steth = testlib::tokens::STETH;

        let bucket_override = TokenBucketFeeOverride {
            tokens: [weth, steth].into_iter().collect(),
            factor: FeeFactor::try_from(0.00001).unwrap(),
        };

        let default_fee = FeeFactor::try_from(0.001).unwrap();
        let volume_fee_policy =
            VolumeFeePolicy::new(vec![bucket_override], Some(default_fee), false, weth);

        // Buying native ETH is classified like buying the wrapped native token,
        // so the bucket applies even though it doesn't list BUY_ETH_ADDRESS.
        let override_ =
            volume_fee_policy.get_applicable_volume_fee_factor(BUY_ETH_ADDRESS, steth, None);
        assert_eq!(override_, Some(FeeFactor::try_from(0.00001).unwrap()));
    }

    #[test]
    fn test_bucket_listing_eth_marker_matches_wrapped_native_token() {
        let weth = testlib::tokens::WETH;
        let steth = testlib::tokens::STETH;

        // The bucket lists the native-ETH marker instead of the wrapped
        // native token.
        let bucket_override = TokenBucketFeeOverride {
            tokens: [BUY_ETH_ADDRESS, steth].into_iter().collect(),
            factor: FeeFactor::try_from(0.00001).unwrap(),
        };

        let default_fee = FeeFactor::try_from(0.001).unwrap();
        let volume_fee_policy =
            VolumeFeePolicy::new(vec![bucket_override], Some(default_fee), false, weth);

        // Both native-ETH buys and wrapped native token buys match the bucket.
        let override_ =
            volume_fee_policy.get_applicable_volume_fee_factor(BUY_ETH_ADDRESS, steth, None);
        assert_eq!(override_, Some(FeeFactor::try_from(0.00001).unwrap()));
        let override_ = volume_fee_policy.get_applicable_volume_fee_factor(weth, steth, None);
        assert_eq!(override_, Some(FeeFactor::try_from(0.00001).unwrap()));
    }

    #[test]
    fn test_same_token_volume_fee_skipped() {
        let weth = testlib::tokens::WETH;
        let dai = testlib::tokens::DAI;
        let default_fee = FeeFactor::try_from(0.001).unwrap();

        let policy = VolumeFeePolicy::new(vec![], Some(default_fee), false, weth);

        // Literal same token: no fee.
        assert_eq!(
            policy.get_applicable_volume_fee_factor(weth, weth, None),
            None
        );
        // Native-equivalent (WETH -> ETH): same token.
        assert_eq!(
            policy.get_applicable_volume_fee_factor(BUY_ETH_ADDRESS, weth, None),
            None
        );
        // Different pair (incl. non-native -> ETH): fee applies.
        assert_eq!(
            policy.get_applicable_volume_fee_factor(weth, dai, None),
            Some(default_fee)
        );
        assert_eq!(
            policy.get_applicable_volume_fee_factor(BUY_ETH_ADDRESS, dai, None),
            Some(default_fee)
        );

        // Opted in: same-token fee is charged.
        let policy = VolumeFeePolicy::new(vec![], Some(default_fee), true, weth);
        assert_eq!(
            policy.get_applicable_volume_fee_factor(weth, weth, None),
            Some(default_fee)
        );
        assert_eq!(
            policy.get_applicable_volume_fee_factor(BUY_ETH_ADDRESS, weth, None),
            Some(default_fee)
        );
    }

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
    fn compute_volume_fee_sub_bps_uses_high_precision() {
        // 0.3 BPS = 0.00003 must not round to zero.
        let fee = compute_volume_fee(U256::from(1_000_000u64), factor(0.00003));
        assert_eq!(fee, U256::from(30u64));
    }

    #[test]
    fn capped_partner_volume_factors_respects_cap_and_skips_non_volume() {
        // Sequence: 100 bps volume (fits), a Surplus policy (ignored, doesn't
        // eat the budget), 300 bps volume (only 200 remaining under 300 cap),
        // 50 bps volume (cap exhausted, dropped).
        let json = br#"{
            "metadata": {
                "partnerFee": [
                    { "recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 100 },
                    { "recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "surplusBps": 500, "maxVolumeBps": 500 },
                    { "recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 300 },
                    { "recipient": "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "volumeBps": 50 }
                ]
            }
        }"#;
        let parsed = app_data::parse(json).unwrap();
        let factors = capped_partner_volume_factors(&parsed, factor(0.03)); // 3% cap
        assert_eq!(factors.len(), 2);
        // First fee: 100 bps = 1% (fits inside 3% cap).
        assert!((factors[0].get() - 0.01).abs() < 1e-9);
        // Second fee: 3% requested, but fees compound as (1+f_1)(1+f_2)
        // and the cap applies to the compounded overhead. With 1% already
        // accumulated the remaining headroom is (1.03/1.01) - 1 ≈ 1.9802%.
        assert!((factors[1].get() - (1.03_f64 / 1.01 - 1.0)).abs() < 1e-9);
    }
}
