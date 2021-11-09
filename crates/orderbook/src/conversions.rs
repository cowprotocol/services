use anyhow::{anyhow, Result};
use bigdecimal::num_bigint::ToBigInt;
use num::bigint::{BigInt, BigUint, Sign};
use primitive_types::{H160, H256, U256};
use sqlx::types::BigDecimal;
use std::convert::TryInto;
// TODO: It would be nice to avoid copying the underlying BigInt when converting BigDecimal to
// anything else but the simple big_decimal.to_bigint makes a copy internally.

pub fn u256_to_big_uint(input: &U256) -> BigUint {
    let mut bytes = [0; 32];
    input.to_big_endian(&mut bytes);
    BigUint::from_bytes_be(&bytes)
}

pub fn u256_to_big_decimal(u256: &U256) -> BigDecimal {
    let big_uint = u256_to_big_uint(u256);
    BigDecimal::from(bigint_04_to_03(BigInt::from(big_uint)))
}

pub fn bigint_to_u256(input: &BigInt) -> Option<U256> {
    let (sign, bytes) = input.to_bytes_be();
    if sign == Sign::Minus || bytes.len() > 32 {
        return None;
    }
    Some(U256::from_big_endian(&bytes))
}

pub fn big_decimal_to_big_uint(big_decimal: &BigDecimal) -> Option<BigUint> {
    bigint_03_to_04(big_decimal.to_bigint()?).try_into().ok()
}

pub fn big_decimal_to_u256(big_decimal: &BigDecimal) -> Option<U256> {
    if !big_decimal.is_integer() {
        return None;
    }
    let big_int = bigint_03_to_04(big_decimal.to_bigint()?);
    bigint_to_u256(&big_int)
}

pub fn h160_from_vec(vec: Vec<u8>) -> Result<H160> {
    let array: [u8; 20] = vec
        .try_into()
        .map_err(|_| anyhow!("h160 has wrong length"))?;
    Ok(H160::from(array))
}

pub fn h256_from_vec(vec: Vec<u8>) -> Result<H256> {
    let array: [u8; 32] = vec
        .try_into()
        .map_err(|_| anyhow!("h256 has wrong length"))?;
    Ok(H256::from(array))
}

fn bigint_03_to_04(input: bigdecimal::num_bigint::BigInt) -> BigInt {
    let (sign, digits) = input.to_u32_digits();
    BigInt::new(
        match sign {
            bigdecimal::num_bigint::Sign::Minus => Sign::Minus,
            bigdecimal::num_bigint::Sign::NoSign => Sign::NoSign,
            bigdecimal::num_bigint::Sign::Plus => Sign::Plus,
        },
        digits,
    )
}

fn bigint_04_to_03(input: BigInt) -> bigdecimal::num_bigint::BigInt {
    let (sign, digits) = input.to_u32_digits();
    bigdecimal::num_bigint::BigInt::new(
        match sign {
            Sign::Minus => bigdecimal::num_bigint::Sign::Minus,
            Sign::NoSign => bigdecimal::num_bigint::Sign::NoSign,
            Sign::Plus => bigdecimal::num_bigint::Sign::Plus,
        },
        digits,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use num::{One, Zero};
    use std::str::FromStr;

    #[test]
    fn u256_to_big_uint_() {
        assert_eq!(u256_to_big_uint(&U256::zero()), BigUint::zero());
        assert_eq!(u256_to_big_uint(&U256::one()), BigUint::one());
        assert_eq!(
            u256_to_big_uint(&U256::MAX),
            BigUint::from_str(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
            )
            .unwrap()
        );
    }

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
    fn bigint_to_u256_() {
        assert_eq!(bigint_to_u256(&BigInt::zero()), Some(U256::zero()));
        assert_eq!(bigint_to_u256(&BigInt::one()), Some(U256::one()));

        let max_u256_as_bigint = BigInt::from_str(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        )
        .unwrap();
        assert_eq!(bigint_to_u256(&max_u256_as_bigint), Some(U256::MAX));

        assert!(bigint_to_u256(&(max_u256_as_bigint + BigInt::one())).is_none());
        assert!(bigint_to_u256(&BigInt::from(-1)).is_none());
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

    #[test]
    fn h160_from_vec_() {
        let valid_input = [1u8; 20].to_vec();
        assert_eq!(
            h160_from_vec(valid_input).unwrap(),
            H160::from_slice(&[1u8; 20])
        );

        let wrong_length = vec![0u8];
        assert_eq!(
            h160_from_vec(wrong_length).unwrap_err().to_string(),
            "h160 has wrong length"
        );
    }

    #[test]
    fn h256_from_vec_() {
        let valid_input = [1u8; 32].to_vec();
        assert_eq!(
            h256_from_vec(valid_input).unwrap(),
            H256::from_slice(&[1u8; 32])
        );

        let wrong_length = vec![0u8];
        assert_eq!(
            h256_from_vec(wrong_length).unwrap_err().to_string(),
            "h256 has wrong length"
        );
    }
}
