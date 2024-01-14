pub use primitive_types::{H160, H256, U256};

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

/// A transaction ID, AKA transaction hash.
#[derive(Clone, Debug)]
pub struct TxId(pub H256);

impl From<H256> for TxId {
    fn from(value: H256) -> Self {
        Self(value)
    }
}

/// Block number.
#[derive(Debug, Clone, Copy)]
pub struct BlockNo(pub u64);

impl From<u64> for BlockNo {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Domain separator used for signing.
///
/// https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
#[derive(Debug, Clone, Copy)]
pub struct DomainSeparator(pub [u8; 32]);
