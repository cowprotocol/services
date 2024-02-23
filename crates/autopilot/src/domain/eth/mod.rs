pub use primitive_types::{H160, H256, U256};

/// An address. Can be an EOA or a smart contract address.
pub type Address = SimpleValue<H160>;

/// Block number.
pub type BlockNo = SimpleValue<u64>;

/// A transaction ID, AKA transaction hash.
pub type TxId = SimpleValue<H256>;

pub type TokenAddress = SimpleValue<H160>;
pub type TokenAmount = SimpleValue<U256>;

/// An amount denominated in the sell token for [`Kind::Sell`], or in
/// the buy token for [`Kind::Buy`].
pub type TargetAmount = SimpleValue<U256>;

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct Asset {
    pub amount: TokenAmount,
    pub token: TokenAddress,
}

/// Domain separator used for signing.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SimpleValue<T>(T);

impl<T> From<T> for SimpleValue<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> std::ops::Deref for SimpleValue<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::Add for SimpleValue<T>
where
    T: std::ops::Add<Output = T>,
{
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl<T> std::ops::AddAssign for SimpleValue<T>
where
    T: std::ops::AddAssign,
{
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl<BigRational> std::ops::Mul for SimpleValue<BigRational>
where
    BigRational: std::ops::Mul<Output = BigRational>,
{
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}
