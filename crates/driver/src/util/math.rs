use ethereum_types::U256;

/// Computes `x * q / d` rounding down.
///
/// Returns `None` if `d` is `0` or if the result overflows a 256-bit integer.
pub fn mul_ratio(x: U256, q: U256, d: U256) -> Option<U256> {
    if d.is_zero() {
        return None;
    }

    // fast path when math in U256 doesn't overflow
    if let Some(res) = x.checked_mul(q) {
        return Some(res / d);
    }

    x.full_mul(q).checked_div(d.into())?.try_into().ok()
}

/// Computes `x * q / d` rounding up.
///
/// Returns `None` if `d` is `0` or if the result overflows a 256-bit integer.
pub fn mul_ratio_ceil(x: U256, q: U256, d: U256) -> Option<U256> {
    if d.is_zero() {
        return None;
    }

    // fast path when math in U256 doesn't overflow
    if let Some(p) = x.checked_mul(q) {
        let (div, rem) = p.div_mod(d);
        return div.checked_add(u8::from(!rem.is_zero()).into());
    }

    let p = x.full_mul(q);
    let (div, rem) = p.div_mod(d.into());
    let result = U256::try_from(div).ok()?;
    result.checked_add(u8::from(!rem.is_zero()).into())
}
