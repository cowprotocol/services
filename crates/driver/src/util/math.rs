use {alloy::primitives::U256, number::u256_ext::U256Ext};

/// Computes `x * q / d` rounding down.
///
/// Returns `None` if `d` is `0` or if the result overflows a 256-bit integer.
pub fn mul_ratio(x: U256, q: U256, d: U256) -> Option<U256> {
    x.checked_mul_ratio(&q, &d)
}

/// Computes `x * q / d` rounding up.
///
/// Returns `None` if `d` is `0` or if the result overflows a 256-bit integer.
pub fn mul_ratio_ceil(x: U256, q: U256, d: U256) -> Option<U256> {
    x.checked_mul_ratio_ceil(&q, &d)
}
