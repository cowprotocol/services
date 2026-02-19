use {
    crate::{arguments::TokenBucketFeeOverride, fee_factor::FeeFactor},
    alloy::primitives::{Address, U256},
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
        // 1. For final amounts that end up close to 0 atoms we always take a fee so we
        //    are not attackable through low decimal tokens.
        // 2. When validating fees this consistently picks the same amount.
        U256::from((fee_in_eth / self.sell_token_price).ceil())
    }
}

pub struct VolumeFeePolicy {
    bucket_overrides: Vec<TokenBucketFeeOverride>,
    default_factor: Option<FeeFactor>,
    enable_sell_equals_buy_volume_fee: bool,
}

impl VolumeFeePolicy {
    pub fn new(
        bucket_overrides: Vec<TokenBucketFeeOverride>,
        default_factor: Option<FeeFactor>,
        enable_sell_equals_buy_volume_fee: bool,
    ) -> Self {
        Self {
            bucket_overrides,
            default_factor,
            enable_sell_equals_buy_volume_fee,
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
        // Skip volume fee for same-token trades if the flag is disabled
        if buy_token == sell_token && !self.enable_sell_equals_buy_volume_fee {
            return None;
        }

        // Check for token bucket overrides first (both tokens must be in the same
        // bucket)
        for fee_override in &self.bucket_overrides {
            if fee_override.tokens.contains(&buy_token) && fee_override.tokens.contains(&sell_token)
            {
                return Some(fee_override.factor);
            }
        }

        // Fall back to default factor either from argument or configured default
        fee_factor.or(self.default_factor)
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::address};

    #[test]
    fn test_volume_fee_bucket_override() {
        let usdc = address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
        let dai = address!("6B175474E89094C44Da98b954EedeAC495271d0F");
        let usdt = address!("dAC17F958D2ee523a2206206994597C13D831ec7");
        let weth = address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

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
}
