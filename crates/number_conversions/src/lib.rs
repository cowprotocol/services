use anyhow::{ensure, Result};
use num::{bigint::Sign, BigInt, BigRational, BigUint, Zero};
use primitive_types::U256;

pub fn u256_to_big_uint(input: &U256) -> BigUint {
    let mut bytes = [0; 32];
    input.to_big_endian(&mut bytes);
    BigUint::from_bytes_be(&bytes)
}

pub fn u256_to_big_int(input: &U256) -> BigInt {
    BigInt::from_biguint(Sign::Plus, u256_to_big_uint(input))
}

pub fn u256_to_big_rational(input: &U256) -> BigRational {
    BigRational::new(u256_to_big_int(input), 1.into())
}

pub fn big_uint_to_u256(input: &BigUint) -> Result<U256> {
    let bytes = input.to_bytes_be();
    ensure!(bytes.len() <= 32, "too large");
    Ok(U256::from_big_endian(&bytes))
}

pub fn big_int_to_u256(input: &BigInt) -> Result<U256> {
    ensure!(input.sign() != Sign::Minus, "negative");
    big_uint_to_u256(input.magnitude())
}

pub fn big_rational_to_u256(ratio: &BigRational) -> Result<U256> {
    ensure!(!ratio.denom().is_zero(), "zero denominator");
    big_int_to_u256(&(ratio.numer() / ratio.denom()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use num::One;
    use std::str::FromStr;

    #[test]
    fn big_integer_to_u256() {
        for val in &[0i32, 42, 1337] {
            assert_eq!(
                big_int_to_u256(&BigInt::from(*val)).unwrap(),
                U256::from(*val),
            );
        }
    }

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
    fn bigint_to_u256_() {
        assert_eq!(big_int_to_u256(&BigInt::zero()).unwrap(), U256::zero());
        assert_eq!(big_int_to_u256(&BigInt::one()).unwrap(), U256::one());
        let max_u256_as_bigint = BigInt::from_str(
            "115792089237316195423570985008687907853269984665640564039457584007913129639935",
        )
        .unwrap();
        assert_eq!(big_int_to_u256(&max_u256_as_bigint).unwrap(), U256::MAX);
        assert!(big_int_to_u256(&(max_u256_as_bigint + BigInt::one())).is_err());
        assert!(big_int_to_u256(&BigInt::from(-1)).is_err());
    }
}
