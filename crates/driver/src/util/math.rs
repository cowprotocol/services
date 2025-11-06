use alloy::primitives::{U256, U512};

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

    let div = (U512::from(x) * U512::from(q)) / U512::from(d);

    let limbs = div.into_limbs();
    if limbs[4..].iter().any(|limb| *limb != 0) {
        return None;
    }

    Some(U256::from_limbs_slice(&limbs[..4]))
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
        let (div, rem) = (p / d, p % d);
        return div.checked_add(U256::from(!rem.is_zero()));
    }

    let p = U512::from(x) * U512::from(q);
    let d = U512::from(d);
    let (div, rem) = (p / d, p % d);

    let limbs = div.into_limbs();
    if limbs[4..].iter().any(|limb| *limb != 0) {
        return None;
    }

    let result = U256::from_limbs_slice(&div.into_limbs()[..4]);
    result.checked_add(U256::from(!rem.is_zero()))
}
