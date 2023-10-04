use ethereum_types::U256;

/// Computes `x * q / d` rounding down.
///
/// Returns `None` if `d` is `0` or if the result overflows a 256-bit integer.
pub fn mul_ratio(x: U256, q: U256, d: U256) -> Option<U256> {
    x.full_mul(q).checked_div(d.into())?.try_into().ok()
}

/// Computes `x * q / d` rounding up.
///
/// Returns `None` if `d` is `0` or if the result overflows a 256-bit integer.
pub fn mul_ratio_ceil(x: U256, q: U256, d: U256) -> Option<U256> {
    let p = x.full_mul(q);
    let result = U256::try_from(p.checked_div(d.into())?).ok()?;
    let round_up = !p.checked_rem(d.into())?.is_zero();
    result.checked_add(u8::from(round_up).into())
}
