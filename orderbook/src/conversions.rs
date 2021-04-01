use anyhow::{anyhow, Result};
use bigdecimal::BigDecimal;
use num_bigint::{BigInt, BigUint, Sign, ToBigInt};
use primitive_types::{H160, H256, U256};
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
    BigDecimal::from(BigInt::from(big_uint))
}

pub fn bigint_to_u256(input: &BigInt) -> Option<U256> {
    let (sign, bytes) = input.to_bytes_be();
    if sign == Sign::Minus || bytes.len() > 32 {
        return None;
    }
    Some(U256::from_big_endian(&bytes))
}

pub fn big_decimal_to_big_uint(big_decimal: &BigDecimal) -> Option<BigUint> {
    big_decimal.to_bigint()?.try_into().ok()
}

pub fn big_decimal_to_u256(big_decimal: &BigDecimal) -> Option<U256> {
    if !big_decimal.is_integer() {
        return None;
    }
    let big_int = big_decimal.to_bigint()?;
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
