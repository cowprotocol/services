use {
    num::{BigInt, BigRational},
    primitive_types::U256,
};

pub trait U256Ext: Sized {
    fn to_big_int(&self) -> BigInt;
    fn to_big_rational(&self) -> BigRational;

    fn checked_ceil_div(&self, other: &Self) -> Option<Self>;
    fn ceil_div(&self, other: &Self) -> Self;
}

impl U256Ext for U256 {
    fn to_big_int(&self) -> BigInt {
        number::conversions::u256_to_big_int(self)
    }

    fn to_big_rational(&self) -> BigRational {
        number::conversions::u256_to_big_rational(self)
    }

    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(1.into())?)?
            .checked_div(*other)
    }

    fn ceil_div(&self, other: &Self) -> Self {
        self.checked_ceil_div(other)
            .expect("ceiling division arithmetic error")
    }
}

impl U256Ext for alloy::primitives::U256 {
    fn to_big_int(&self) -> BigInt {
        number::conversions::alloy::u256_to_big_int(self)
    }

    fn to_big_rational(&self) -> BigRational {
        number::conversions::alloy::u256_to_big_rational(self)
    }

    fn checked_ceil_div(&self, other: &Self) -> Option<Self> {
        self.checked_add(other.checked_sub(alloy::primitives::U256::ONE)?)?
            .checked_div(*other)
    }

    fn ceil_div(&self, other: &Self) -> Self {
        self.checked_ceil_div(other)
            .expect("ceiling division arithmetic error")
    }
}
