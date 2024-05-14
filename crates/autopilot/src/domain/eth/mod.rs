use derive_more::{From, Into};
pub use primitive_types::{H160, H256, U256};

/// An address. Can be an EOA or a smart contract address.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct Address(pub H160);

/// Block number.
#[derive(Debug, Copy, Clone, From)]
pub struct BlockNo(pub u64);

/// A transaction ID, AKA transaction hash.
#[derive(Debug, Copy, Clone, From)]
pub struct TxId(pub H256);

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From)]
pub struct TokenAddress(pub H160);

/// An ERC20 token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, From, Into)]
pub struct TokenAmount(pub U256);

impl TokenAmount {
    /// Applies a factor to the token amount.
    ///
    /// The factor is first multiplied by 10^18 to convert it to integer, to
    /// avoid rounding to 0. Then, the token amount is divided by 10^18 to
    /// convert it back to the original scale.
    ///
    /// The higher the conversion factor (10^18) the precision is higher. E.g.
    /// 0.123456789123456789 will be converted to 123456789123456789.
    pub fn apply_factor(&self, factor: f64) -> Option<Self> {
        Some(
            (self
                .0
                .checked_mul(U256::from_f64_lossy(factor * 1000000000000000000.))?
                / 1000000000000000000u128)
                .into(),
        )
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

impl std::ops::Add for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl num::CheckedAdd for TokenAmount {
    fn checked_add(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(other.0).map(Into::into)
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

impl std::ops::AddAssign for TokenAmount {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl num::Zero for TokenAmount {
    fn zero() -> Self {
        Self(U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Asset {
    pub amount: TokenAmount,
    pub token: TokenAddress,
}

/// An amount of native Ether tokens denominated in wei.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, From, Into)]
pub struct Ether(pub U256);

impl std::ops::Add for Ether {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl num::Zero for Ether {
    fn zero() -> Self {
        Self(U256::zero())
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

/// Domain separator used for signing.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);

/// Originated from the blockchain transaction input data.
pub type Calldata = crate::util::Bytes<Vec<u8>>;
