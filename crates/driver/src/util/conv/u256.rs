use {crate::domain::eth, bigdecimal::Zero};

fn to_big_uint(value: eth::U256) -> num::BigUint {
    let mut bytes = [0; 32];
    value.to_big_endian(&mut bytes);
    num::BigUint::from_bytes_be(&bytes)
}

fn to_big_int(value: eth::U256) -> num::BigInt {
    num::BigInt::from_biguint(num::bigint::Sign::Plus, to_big_uint(value))
}

pub fn to_big_rational(value: eth::U256) -> num::BigRational {
    num::BigRational::new(to_big_int(value), 1.into())
}

fn from_big_uint(input: &num::BigUint) -> eth::U256 {
    let bytes = input.to_bytes_be();
    assert!(bytes.len() <= 32, "too large");
    eth::U256::from_big_endian(&bytes)
}

fn from_big_int(input: &num::BigInt) -> eth::U256 {
    assert!(input.sign() != num::bigint::Sign::Minus, "negative");
    from_big_uint(input.magnitude())
}

pub fn from_big_rational(value: &num::BigRational) -> eth::U256 {
    assert!(*value.denom() != num::BigInt::zero(), "zero denominator");
    from_big_int(&(value.numer() / value.denom()))
}
