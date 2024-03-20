use {crate::util::bytes::Bytes, web3::types::AccessList};

mod chain;

pub use {
    self::chain::ChainId,
    ethereum_types::{H160, H256, U256},
};

/// A contract address.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContractAddress(pub H160);

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenAddress(pub H160);

impl From<H160> for TokenAddress {
    fn from(inner: H160) -> Self {
        Self(inner)
    }
}

/// The WETH token (or equivalent) for the EVM compatible network.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WethAddress(pub H160);

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub amount: U256,
    pub token: TokenAddress,
}

/// An Ether amount in wei.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ether(pub U256);

impl From<U256> for Ether {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

/// Gas amount.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gas(pub U256);

impl std::ops::Add for Gas {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

/// A 256-bit rational type.
pub type Rational = num::rational::Ratio<U256>;

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

/// An onchain transaction.
#[derive(Debug, Clone)]
pub struct Tx {
    pub from: Address,
    pub to: Address,
    pub value: Ether,
    pub input: Bytes<Vec<u8>>,
    pub access_list: AccessList,
}
