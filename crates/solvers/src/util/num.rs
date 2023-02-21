//! Numeric and arithmetic utilities.

use {
    bigdecimal::BigDecimal,
    num::{BigInt, Signed},
};

/// Round a [`bigdecimal::BigDecimal`] to the specified precision.
///
/// This is the same as [`bigdecimal::BigDecimal::round`], but it does not panic
/// for high precision values.
pub fn round(x: &BigDecimal, round_digits: i64) -> BigDecimal {
    // Adapted from <https://docs.rs/bigdecimal/0.3.0/src/bigdecimal/lib.rs.html#587-610>

    let (bigint, decimal_part_digits) = x.as_bigint_and_exponent();
    let need_to_round_digits = decimal_part_digits - round_digits;
    if round_digits >= 0 && need_to_round_digits <= 0 {
        return x.clone();
    }

    let mut number = bigint.clone();
    if number.is_negative() {
        number = -number;
    }
    for _ in 0..(need_to_round_digits - 1) {
        number /= 10;
    }
    let digit = number % 10;

    if digit <= BigInt::from(4) {
        x.with_scale(round_digits)
    } else if bigint.is_negative() {
        x.with_scale(round_digits) - BigDecimal::new(BigInt::from(1), round_digits)
    } else {
        x.with_scale(round_digits) + BigDecimal::new(BigInt::from(1), round_digits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn bigdecimal_round_panics() {
        let value =
            "42.115792089237316195423570985008687907853269984665640564039457584007913129639935"
                .parse::<BigDecimal>()
                .unwrap();

        let _ = value.round(4);
    }

    #[test]
    fn round_does_not_panic() {
        let value =
            "42.115792089237316195423570985008687907853269984665640564039457584007913129639935"
                .parse::<BigDecimal>()
                .unwrap();

        assert_eq!(round(&value, 4), "42.1158".parse::<BigDecimal>().unwrap());
    }
}
