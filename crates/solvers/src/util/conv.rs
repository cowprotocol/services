//! Conversion utilities.

use crate::domain::eth;
use bigdecimal::BigDecimal;
use ethereum_types::U256;

/// Converts a `BigDecimal` value to a `eth::Rational` value.
pub fn decimal_to_rational(d: &BigDecimal) -> Option<eth::Rational> {
    let (int, exp) = d.as_bigint_and_exponent();

    let numer = {
        let (sign, bytes) = int.to_bytes_be();
        if sign == num::bigint::Sign::Minus || bytes.len() > 32 {
            return None;
        }

        let multiplier = if exp < 0 {
            U256::from(10_u8).checked_pow(exp.abs().into())?
        } else {
            U256::one()
        };

        U256::from_big_endian(&bytes).checked_mul(multiplier)?
    };
    let denom = if exp > 0 {
        U256::from(10_u8).checked_pow(exp.into())?
    } else {
        U256::one()
    };

    Some(eth::Rational::new_raw(numer, denom))
}
