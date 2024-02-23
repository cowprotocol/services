pub use primitive_types::{H160, H256, U256};

/// An address. Can be an EOA or a smart contract address.
pub type Address = SimpleValue<H160>;

/// Block number.
pub type BlockNo = SimpleValue<u64>;

/// A transaction ID, AKA transaction hash.
pub type TxId = SimpleValue<H256>;

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
