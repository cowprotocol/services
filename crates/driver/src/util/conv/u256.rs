use {crate::domain::eth, anyhow::Result, bigdecimal::Zero};

pub trait U256Ext: Sized {
    fn to_big_int(&self) -> num::BigInt;
    fn to_big_uint(&self) -> num::BigUint;
    fn to_big_rational(&self) -> num::BigRational;

    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;
    fn ceil_div(&self, other: &Self) -> Self;

    fn from_big_int(input: &num::BigInt) -> Result<Self>;
    fn from_big_uint(input: &num::BigUint) -> Result<Self>;
    fn from_big_rational(value: &num::BigRational) -> Result<Self>;
}

impl U256Ext for eth::U256 {
    fn to_big_int(&self) -> num::BigInt {
        num::BigInt::from_biguint(num::bigint::Sign::Plus, self.to_big_uint())
    }

    fn to_big_uint(&self) -> num::BigUint {
        let mut bytes = [0; 32];
        self.to_big_endian(&mut bytes);
        num::BigUint::from_bytes_be(&bytes)
    }

    fn to_big_rational(&self) -> num::BigRational {
        num::BigRational::new(self.to_big_int(), 1.into())
    }

    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }

    fn ceil_div(&self, other: &Self) -> Self {
        self.checked_ceil_div(other)
            .expect("ceiling division arithmetic error")
    }

    fn from_big_int(input: &num::BigInt) -> Result<eth::U256> {
        anyhow::ensure!(input.sign() != num::bigint::Sign::Minus, "negative");
        Self::from_big_uint(input.magnitude())
    }

    fn from_big_uint(input: &num::BigUint) -> Result<Self> {
        let bytes = input.to_bytes_be();
        anyhow::ensure!(bytes.len() <= 32, "too large");
        Ok(eth::U256::from_big_endian(&bytes))
    }

    fn from_big_rational(value: &num::BigRational) -> Result<Self> {
        anyhow::ensure!(*value.denom() != num::BigInt::zero(), "zero denominator");
        Self::from_big_int(&(value.numer() / value.denom()))
    }
}
