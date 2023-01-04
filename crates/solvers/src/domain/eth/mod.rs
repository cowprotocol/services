use ethereum_types::{H160, U256};
use std::ops::Deref;

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub H160);

impl Deref for TokenAddress {
    type Target = H160;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

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
