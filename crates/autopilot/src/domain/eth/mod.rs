pub use primitive_types::{H160, H256, U256};

/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
pub const ETH_TOKEN: TokenAddress = TokenAddress(H160([0xee; 20]));

/// An address. Can be an EOA or a smart contract address.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub H160);

impl From<H160> for Address {
    fn from(value: H160) -> Self {
        Self(value)
    }
}

impl From<Address> for H160 {
    fn from(value: Address) -> Self {
        value.0
    }
}

/// Block number.
#[derive(Debug, Copy, Clone)]
pub struct BlockNo(pub u64);

impl From<u64> for BlockNo {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// A transaction ID, AKA transaction hash.
#[derive(Debug, Copy, Clone)]
pub struct TxId(pub H256);

impl From<H256> for TxId {
    fn from(value: H256) -> Self {
        Self(value)
    }
}

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub H160);

impl TokenAddress {
    /// If the token is ETH, return WETH, thereby "wrapping" it.
    pub fn wrap(self, weth: WethAddress) -> Self {
        if self == ETH_TOKEN {
            weth.into()
        } else {
            self
        }
    }
}

/// The address of the WETH contract.
#[derive(Debug, Clone, Copy)]
pub struct WethAddress(pub TokenAddress);

impl From<WethAddress> for TokenAddress {
    fn from(value: WethAddress) -> Self {
        value.0
    }
}

impl From<H160> for WethAddress {
    fn from(value: H160) -> Self {
        WethAddress(value.into())
    }
}

impl From<H160> for TokenAddress {
    fn from(value: H160) -> Self {
        Self(value.into())
    }
}

impl From<TokenAddress> for H160 {
    fn from(value: TokenAddress) -> Self {
        value.0.into()
    }
}

/// An ERC20 token amount.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAmount(pub U256);

impl From<U256> for TokenAmount {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

impl From<TokenAmount> for U256 {
    fn from(value: TokenAmount) -> Self {
        value.0
    }
}

impl From<u128> for TokenAmount {
    fn from(value: u128) -> Self {
        Self(value.into())
    }
}

impl std::ops::Add for TokenAmount {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
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
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Ether(pub U256);

impl From<U256> for Ether {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

impl From<Ether> for U256 {
    fn from(value: Ether) -> Self {
        value.0
    }
}

/// Domain separator used for signing.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);
