use alloy::primitives::{U256, U512, ruint::UintTryFrom};

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

    // SAFETY: at this point !d.is_zero() upholds
    let div = (x.widening_mul(q)) / U512::from(d);
    U256::uint_try_from(div).ok()
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

    let p = x.widening_mul(q);
    let d = U512::from(d);
    // SAFETY: at this point !d.is_zero() upholds
    let (div, rem) = (p / d, p % d);

    let result = U256::uint_try_from(div).ok()?;
    result.checked_add(U256::from(!rem.is_zero()))
}

#[cfg(test)]
mod test {
    use {
        crate::util::math::{mul_ratio, mul_ratio_ceil},
        alloy::primitives::U256,
    };

    #[test]
    fn mul_ratio_zero() {
        assert!(mul_ratio(U256::from(10), U256::from(10), U256::ZERO).is_none());
    }

    #[test]
    fn mul_ratio_overflow() {
        assert!(mul_ratio(U256::MAX, U256::from(2), U256::ONE).is_none());
    }

    #[test]
    fn mul_ratio_ceil_zero() {
        assert!(mul_ratio_ceil(U256::from(10), U256::from(10), U256::ZERO).is_none());
    }

    #[test]
    fn mul_ratio_ceil_overflow() {
        assert!(mul_ratio_ceil(U256::MAX, U256::from(2), U256::ONE).is_none());
    }

    #[test]
    fn mul_ratio_normal() {
        // Exact division
        assert_eq!(
            mul_ratio(U256::from(100), U256::from(5), U256::from(10)),
            Some(U256::from(50))
        );

        // Division with remainder (rounds down)
        assert_eq!(
            mul_ratio(U256::from(100), U256::from(3), U256::from(10)),
            Some(U256::from(30))
        );

        // Large values that don't overflow
        assert_eq!(
            mul_ratio(U256::from(u128::MAX), U256::from(2), U256::from(4)),
            Some(U256::from(u128::MAX / 2))
        );
    }

    #[test]
    fn mul_ratio_ceil_normal() {
        // Exact division (no rounding needed)
        assert_eq!(
            mul_ratio_ceil(U256::from(100), U256::from(5), U256::from(10)),
            Some(U256::from(50))
        );

        // Division with remainder (rounds up)
        assert_eq!(
            mul_ratio_ceil(U256::from(10), U256::from(3), U256::from(4)),
            Some(U256::from(8))
        );

        // Large values that don't overflow
        assert_eq!(
            mul_ratio_ceil(U256::from(u128::MAX), U256::from(2), U256::from(4)),
            Some(U256::from(u128::MAX / 2 + 1))
        );
    }
}
