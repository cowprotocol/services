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

/// Gas amount.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gas(pub U256);

/// A 256-bit rational type.
pub type Rational = num::rational::Ratio<U256>;
