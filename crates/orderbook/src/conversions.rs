use bigdecimal::num_bigint::ToBigInt;
use num::bigint::{BigInt, BigUint};
use primitive_types::U256;
use sqlx::types::BigDecimal;
use std::convert::TryInto;

// TODO: It would be nice to avoid copying the underlying BigInt when converting BigDecimal to
// anything else but the simple big_decimal.to_bigint makes a copy internally.

pub fn u256_to_big_decimal(u256: &U256) -> BigDecimal {
    let big_uint = number_conversions::u256_to_big_uint(u256);
    BigDecimal::from(BigInt::from(big_uint))
}

pub fn big_decimal_to_big_uint(big_decimal: &BigDecimal) -> Option<BigUint> {
    big_decimal.to_bigint()?.try_into().ok()
}

pub fn big_decimal_to_u256(big_decimal: &BigDecimal) -> Option<U256> {
    if !big_decimal.is_integer() {
        return None;
    }
    let big_int = big_decimal.to_bigint()?;
    number_conversions::big_int_to_u256(&big_int).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use num::{One, Zero};
    use std::str::FromStr;

    #[test]
    fn u256_to_big_decimal_() {
        assert_eq!(u256_to_big_decimal(&U256::zero()), BigDecimal::zero());
        assert_eq!(u256_to_big_decimal(&U256::one()), BigDecimal::one());
        assert_eq!(
            u256_to_big_decimal(&U256::MAX),
            BigDecimal::from_str(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
            .unwrap()
        );
    }

    #[test]
    fn big_decimal_to_big_uint_() {
        assert_eq!(
            big_decimal_to_big_uint(&BigDecimal::zero()),
            Some(BigUint::zero())
        );
        assert_eq!(
            big_decimal_to_big_uint(&BigDecimal::one()),
            Some(BigUint::one())
        );
        assert!(big_decimal_to_big_uint(
            &BigDecimal::from_str(
                "9115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
            .unwrap()
        )
        .is_some());

        assert!(big_decimal_to_big_uint(&BigDecimal::from(-1)).is_none());
        assert!(big_decimal_to_u256(&BigDecimal::from_str("0.5").unwrap()).is_none());
    }

    #[test]
    fn big_decimal_to_u256_() {
        assert_eq!(big_decimal_to_u256(&BigDecimal::zero()), Some(U256::zero()));
        assert_eq!(big_decimal_to_u256(&BigDecimal::one()), Some(U256::one()));
        assert!(big_decimal_to_u256(&BigDecimal::from(-1)).is_none());
        assert!(big_decimal_to_u256(&BigDecimal::from_str("0.5").unwrap()).is_none());
        let max_u256_as_big_decimal = BigDecimal::from_str(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        )
        .unwrap();
        assert_eq!(
            big_decimal_to_u256(&max_u256_as_big_decimal),
            Some(U256::MAX)
        );
        assert!(big_decimal_to_u256(&(max_u256_as_big_decimal + BigDecimal::one())).is_none());
    }
}
