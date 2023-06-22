pub use primitive_types::{H160, U256};

/// ERC20 token address for ETH. In reality, ETH is not an ERC20 token because
/// it does not implement the ERC20 interface, but this address is used by
/// convention across the Ethereum ecosystem whenever ETH is treated like an
/// ERC20 token.
pub const ETH_TOKEN: TokenAddress = TokenAddress(H160([0xee; 20]));

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub H160);

/// Gas amount denominated in wei.
#[derive(Debug, Clone, Copy)]
pub struct Gas(pub U256);

impl From<U256> for Gas {
    fn from(value: U256) -> Self {
        Self(value)
    }
}
