use {
    alloy_primitives::U256,
    derive_more::{From, Into},
    number::u256_ext::U256Ext,
};

/// An ERC20 token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct TokenAmount(pub U256);

impl TokenAmount {
    /// Applies a factor to the token amount.
    pub fn apply_factor(&self, factor: f64) -> Option<Self> {
        Some(self.0.checked_mul_f64(factor)?.into())
    }
}

/// A value denominated in an order's surplus token (buy token for
/// sell orders and sell token for buy orders).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct SurplusTokenAmount(pub U256);

/// An ERC20 sell token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct SellTokenAmount(pub U256);

impl From<TokenAmount> for SellTokenAmount {
    fn from(value: TokenAmount) -> Self {
        Self(value.0)
    }
}

impl From<SellTokenAmount> for TokenAmount {
    fn from(value: SellTokenAmount) -> Self {
        Self(value.0)
    }
}

impl std::ops::Sub<Self> for TokenAmount {
    type Output = TokenAmount;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.sub(rhs.0).into()
    }
}

impl num::CheckedSub for TokenAmount {
    fn checked_sub(&self, other: &Self) -> Option<Self> {
        self.0.checked_sub(other.0).map(Into::into)
    }
}

impl std::ops::Mul<Self> for TokenAmount {
    type Output = TokenAmount;

    fn mul(self, rhs: Self) -> Self::Output {
        self.0.mul(rhs.0).into()
    }
}

impl num::CheckedMul for TokenAmount {
    fn checked_mul(&self, other: &Self) -> Option<Self> {
        self.0.checked_mul(other.0).map(Into::into)
    }
}

impl num::CheckedAdd for TokenAmount {
    fn checked_add(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Into::into)
    }
}

impl std::ops::Div<Self> for TokenAmount {
    type Output = TokenAmount;

    fn div(self, rhs: Self) -> Self::Output {
        self.0.div(rhs.0).into()
    }
}

impl num::CheckedDiv for TokenAmount {
    fn checked_div(&self, other: &Self) -> Option<Self> {
        self.0.checked_div(other.0).map(Into::into)
    }
}

impl From<u128> for TokenAmount {
    fn from(value: u128) -> Self {
        Self(U256::from(value))
    }
}

impl std::ops::Add for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for TokenAmount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl num::Zero for TokenAmount {
    fn zero() -> Self {
        Self(U256::ZERO)
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl std::fmt::Display for TokenAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
