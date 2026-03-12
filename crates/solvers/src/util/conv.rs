//! Conversion utilities for `eth::Ether` and `eth::Rational`.

use {
    crate::domain::eth,
    bigdecimal::BigDecimal,
    num::{BigInt, One},
    number::conversions::{big_decimal_to_big_rational, big_int_to_u256, u256_to_big_uint},
};

/// Converts a `BigDecimal` value to a `eth::Rational` value. Returns `None` if
/// the specified decimal value cannot be represented as a rational of `U256`
/// integers.
pub fn decimal_to_rational(d: &BigDecimal) -> Option<eth::Rational> {
    let ratio = big_decimal_to_big_rational(d);
    let numer = big_int_to_u256(ratio.numer()).ok()?;
    let denom = big_int_to_u256(ratio.denom()).ok()?;
    Some(eth::Rational::new_raw(numer, denom))
}

/// Converts a `BigDecimal` amount in Ether units to wei.
pub fn decimal_to_ether(d: &BigDecimal) -> Option<eth::Ether> {
    let scaled = d * BigDecimal::new(BigInt::one(), -18);
    let ratio = decimal_to_rational(&scaled)?;
    Some(eth::Ether(ratio.numer() / ratio.denom()))
}

/// Converts an `eth::Ether` amount into a `BigDecimal` representation.
pub fn ether_to_decimal(e: &eth::Ether) -> BigDecimal {
    BigDecimal::new(u256_to_big_uint(&e.0).into(), 18)
}

#[cfg(test)]
mod tests {
    use {super::*, alloy::primitives::U256};

    #[test]
    fn decimal_to_rational_conversions() {
        for (value, numer, denom) in [
            ("4.2", 21, 5),
            (
                "1000.00000000000000000000000000000000000000000000000000000000000\
                 0000000000000000000000000000000000000000000000000000000000000000",
                1000,
                1,
            ),
            ("0.003", 3, 1000),
        ] {
            let result = decimal_to_rational(&value.parse().unwrap()).unwrap();
            assert_eq!(u64::try_from(result.numer()).unwrap(), numer);
            assert_eq!(u64::try_from(result.denom()).unwrap(), denom);
        }
    }

    #[test]
    fn invalid_decimal_to_rational_conversions() {
        for value in [
            // negative
            "-0.42",
            // overflow numerator
            "1111111111111111111111111111111111111111111111111111111111111111111111111111111.1",
            // overflow denominator
            "0.0000000000000000000000000000000000000000000000000000000000000000000000000000001",
        ] {
            let result = decimal_to_rational(&value.parse().unwrap());
            assert!(result.is_none());
        }
    }

    #[test]
    fn decimal_to_and_from_ether() {
        for (decimal, ether) in [
            ("0.01", 10_000_000_000_000_000_u128),
            ("4.20", 4_200_000_000_000_000_000),
            ("10", 10_000_000_000_000_000_000),
        ] {
            let decimal = decimal.parse().unwrap();
            let ether = eth::Ether(U256::from(ether));

            assert_eq!(decimal_to_ether(&decimal).unwrap(), ether);
            assert_eq!(ether_to_decimal(&ether), decimal);
        }
    }
}
