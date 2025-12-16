use {
    crate::arguments::{FeeFactor, TokenBucketFeeOverride},
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

/// Finds the applicable volume fee bucket override for a token pair.
/// Returns None if no override is found.
/// Both tokens must be in the same bucket for the override to apply.
pub fn get_volume_fee_bucket_override(
    bucket_overrides: &[TokenBucketFeeOverride],
    buy_token: Address,
    sell_token: Address,
) -> Option<FeeFactor> {
    // Find token bucket overrides (both tokens must be in the same bucket)
    for fee_override in bucket_overrides {
        if fee_override.tokens.contains(&buy_token) && fee_override.tokens.contains(&sell_token) {
            return Some(fee_override.factor);
        }
    }
    None
}

/// Determines the applicable volume fee factor for a token pair, considering
/// same-token trade configuration, token bucket overrides and default fee
/// factor.
pub fn get_applicable_volume_fee_factor(
    bucket_overrides: &[TokenBucketFeeOverride],
    buy_token: Address,
    sell_token: Address,
    enable_sell_equals_buy_volume_fee: bool,
    default_factor: Option<FeeFactor>,
) -> Option<FeeFactor> {
    // Skip volume fee for same-token trades if the flag is disabled
    if buy_token == sell_token && !enable_sell_equals_buy_volume_fee {
        return None;
    }

    // Check for token bucket overrides first, then fall back to default
    get_volume_fee_bucket_override(bucket_overrides, buy_token, sell_token).or(default_factor)
}
