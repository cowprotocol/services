pub use primitive_types::{H160, U256};

// TODO Don't allow dead code
/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
#[allow(dead_code)]
pub const ETH_TOKEN: TokenAddress = TokenAddress(H160([0xee; 20]));

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub H160);

/// Gas amount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Gas(pub U256);

impl From<U256> for Gas {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

/// Gas price denominated in wei.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GasPrice(pub U256);

impl From<U256> for GasPrice {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

/// An amount of Ether tokens denominated in wei.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Ether(pub U256);

impl std::ops::Mul<Gas> for GasPrice {
    type Output = Ether;

    fn mul(self, rhs: Gas) -> Self::Output {
        Ether(self.0 * rhs.0)
    }
}
