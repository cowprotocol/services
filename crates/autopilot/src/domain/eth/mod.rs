pub use {
    number::nonzero::U256 as NonZeroU256,
    primitive_types::{H160, H256, U256},
};

/// Chain ID as defined by EIP-155.
///
/// https://eips.ethereum.org/EIPS/eip-155
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ChainId(pub U256);

impl From<U256> for ChainId {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub struct NetworkId(pub String);

impl From<String> for NetworkId {
    fn from(value: String) -> Self {
        Self(value)
    }
}
