use {
    alloy_primitives::U256,
    derive_more::derive::{Display, From, Into},
};

/// An amount of native Ether tokens denominated in wei.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    From,
    Into,
    Display,
    Default,
    derive_more::Add,
    derive_more::Sub,
)]
pub struct Ether(pub U256);

impl From<i32> for Ether {
    fn from(value: i32) -> Self {
        Self(U256::from(value))
    }
}

impl From<Ether> for num::BigInt {
    fn from(value: Ether) -> Self {
        num::BigUint::from_bytes_be(value.0.to_be_bytes::<32>().as_slice()).into()
    }
}

impl num::Saturating for Ether {
    fn saturating_add(self, v: Self) -> Self {
        Self(self.0.saturating_add(v.0))
    }

    fn saturating_sub(self, v: Self) -> Self {
        Self(self.0.saturating_sub(v.0))
    }
}

impl num::CheckedSub for Ether {
    fn checked_sub(&self, v: &Self) -> Option<Self> {
        self.0.checked_sub(v.0).map(Ether)
    }
}

impl num::Zero for Ether {
    fn zero() -> Self {
        Self(U256::ZERO)
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl std::iter::Sum for Ether {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(num::Zero::zero(), std::ops::Add::add)
    }
}
