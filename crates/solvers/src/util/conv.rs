//! Conversion utilities.

use {
    crate::domain::eth,
    bigdecimal::BigDecimal,
    ethereum_types::U256,
    num::{rational::Ratio, BigUint},
};

/// Converts a `BigDecimal` value to a `eth::Rational` value. Returns `None` if
/// the specified decimal value cannot be represented as a rational of `U256`
/// integers.
pub fn decimal_to_rational(d: &BigDecimal) -> Option<eth::Rational> {
    let (int, exp) = d.as_bigint_and_exponent();

    // First convert to a `Ratio<BigUint>`. This ensures that the ratio is
    // normalized (i.e. GCD of numerator and denomninator is 1) before trying to
    // convert the components to `U256`s. This allows values like `1.00...000`
    // that would otherwise overflow a `U256` numerator.
    let uint = int.to_biguint()?;
    let factor = BigUint::from(10_u8).pow(exp.unsigned_abs().try_into().ok()?);
    let ratio = if exp >= 0 {
        Ratio::new(uint, factor)
    } else {
        Ratio::new(uint * factor, num::one())
    };

    let numer = biguint_to_u256(ratio.numer())?;
    let denom = biguint_to_u256(ratio.denom())?;

    Some(eth::Rational::new_raw(numer, denom))
}

fn biguint_to_u256(i: &BigUint) -> Option<U256> {
    let bytes = i.to_bytes_be();
    if bytes.len() > 32 {
        return None;
    }
    Some(U256::from_big_endian(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            assert_eq!(result.numer().as_u64(), numer);
            assert_eq!(result.denom().as_u64(), denom);
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
}
