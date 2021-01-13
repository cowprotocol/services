use bigdecimal::BigDecimal;
use num_bigint::{BigInt, Sign, ToBigInt};
use primitive_types::U256;

pub fn u256_to_big_int(input: &U256) -> BigInt {
    let mut bytes = [0; 32];
    input.to_big_endian(&mut bytes);
    BigInt::from_bytes_be(Sign::Plus, &bytes)
}

pub fn u256_to_big_decimal(u256: &U256) -> BigDecimal {
    let big_int = u256_to_big_int(u256);
    BigDecimal::from(big_int)
}

pub fn bigint_to_u256(input: &BigInt) -> Option<U256> {
    let (sign, bytes) = input.to_bytes_be();
    if sign == Sign::Minus || bytes.len() > 32 {
        return None;
    }
    Some(U256::from_big_endian(&bytes))
}

pub fn big_decimal_to_u256(big_decimal: &BigDecimal) -> Option<U256> {
    if !big_decimal.is_integer() {
        return None;
    }
    let big_int = big_decimal.to_bigint()?;
    bigint_to_u256(&big_int)
}
