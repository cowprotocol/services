use ethereum_types::{H160, U256};

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TokenAddress(pub H160);

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

/// Gas amount.
#[derive(Debug, Default, Clone, Copy)]
pub struct Gas(pub U256);

/// A 256-bit rational type.
pub type Rational = num::rational::Ratio<U256>;
