use {
    itertools::Itertools,
    std::collections::{HashMap, HashSet},
};

pub mod allowance;
mod eip712;

pub use {
    allowance::Allowance,
    eip712::{DomainFields, DomainSeparator},
    primitive_types::{H160, H256, U256},
};

// TODO This module is getting a little hectic with all kinds of different
// types, I wonder if there could be meaningful submodules?

/// Chain ID as defined by EIP-155.
///
/// https://eips.ethereum.org/EIPS/eip-155
#[derive(Debug, Clone, Copy)]
pub struct ChainId(pub U256);

impl From<U256> for ChainId {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

/// Chain ID as defined by EIP-155.
///
/// https://eips.ethereum.org/EIPS/eip-155
#[derive(Debug, Clone)]
pub struct NetworkId(pub String);

impl NetworkId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for NetworkId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl std::fmt::Display for NetworkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Gas amount.
#[derive(Debug, Default, Clone, Copy)]
pub struct Gas(pub U256);

impl From<U256> for Gas {
    fn from(value: U256) -> Self {
        Self(value)
    }
}

impl From<u64> for Gas {
    fn from(value: u64) -> Self {
        Self(value.into())
    }
}

impl From<Gas> for U256 {
    fn from(value: Gas) -> Self {
        value.0
    }
}

/// `effective_gas_price` as defined by EIP-1559.
///
/// https://eips.ethereum.org/EIPS/eip-1559#specification
#[derive(Debug, Clone, Copy)]
pub struct EffectiveGasPrice(pub Ether);

impl From<U256> for EffectiveGasPrice {
    fn from(value: U256) -> Self {
        Self(value.into())
    }
}

impl From<EffectiveGasPrice> for U256 {
    fn from(value: EffectiveGasPrice) -> Self {
        value.0.into()
    }
}

/// An EIP-2930 access list. This type ensures that the addresses and storage
/// keys are not repeated, and that the ordering is deterministic.
///
/// https://eips.ethereum.org/EIPS/eip-2930
#[derive(Debug, Clone, Default)]
pub struct AccessList(HashMap<Address, HashSet<StorageKey>>);

impl AccessList {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StorageKey(pub H256);

impl From<H256> for StorageKey {
    fn from(inner: H256) -> Self {
        Self(inner)
    }
}

impl AccessList {
    /// Merge two access lists together.
    pub fn merge(mut self, other: Self) -> Self {
        for (address, storage_keys) in other.0.into_iter() {
            self.0.entry(address).or_default().extend(storage_keys);
        }
        self
    }
}

impl From<web3::types::AccessList> for AccessList {
    fn from(list: web3::types::AccessList) -> Self {
        Self(
            list.into_iter()
                .map(|item| {
                    (
                        item.address.into(),
                        item.storage_keys
                            .into_iter()
                            .map(|key| key.into())
                            .collect(),
                    )
                })
                .collect(),
        )
    }
}

impl From<AccessList> for web3::types::AccessList {
    fn from(list: AccessList) -> Self {
        list.0
            .into_iter()
            .sorted_by_key(|&(address, _)| address)
            .map(|(address, storage_keys)| web3::types::AccessListItem {
                address: address.into(),
                storage_keys: storage_keys.into_iter().sorted().map(|key| key.0).collect(),
            })
            .collect()
    }
}

/// An address. Can be an EOA or a smart contract address.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub H160);

impl From<H160> for Address {
    fn from(inner: H160) -> Self {
        Self(inner)
    }
}

impl From<Address> for H160 {
    fn from(address: Address) -> Self {
        address.0
    }
}

// TODO This type should probably use Ethereum::is_contract to verify during
// construction that it does indeed point to a contract
/// A smart contract address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContractAddress(pub H160);

impl From<H160> for ContractAddress {
    fn from(value: H160) -> Self {
        Self(value)
    }
}

impl From<ContractAddress> for H160 {
    fn from(value: ContractAddress) -> Self {
        value.0
    }
}

impl From<ContractAddress> for ethereum_types::H160 {
    fn from(contract: ContractAddress) -> Self {
        contract.0 .0.into()
    }
}

impl From<ContractAddress> for Address {
    fn from(contract: ContractAddress) -> Self {
        contract.0.into()
    }
}

/// An ERC20 token address.
///
/// https://eips.ethereum.org/EIPS/eip-20
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenAddress(pub ContractAddress);

impl From<H160> for TokenAddress {
    fn from(inner: H160) -> Self {
        Self(inner.into())
    }
}

impl From<TokenAddress> for H160 {
    fn from(token: TokenAddress) -> Self {
        token.0.into()
    }
}

impl From<TokenAddress> for ContractAddress {
    fn from(token: TokenAddress) -> Self {
        token.0
    }
}

/// An asset on the Ethereum blockchain. Represents a particular amount of a
/// particular token.
#[derive(Debug, Clone, Copy)]
pub struct Asset {
    pub amount: U256,
    pub token: TokenAddress,
}

/// An amount of native Ether tokens denominated in wei.
#[derive(Debug, Clone, Copy)]
pub struct Ether(pub U256);

impl From<U256> for Ether {
    fn from(inner: U256) -> Self {
        Self(inner)
    }
}

impl From<Ether> for num::BigInt {
    fn from(ether: Ether) -> Self {
        let mut bytes = [0; 32];
        ether.0.to_big_endian(&mut bytes);
        num::BigUint::from_bytes_be(&bytes).into()
    }
}

impl From<Ether> for U256 {
    fn from(ether: Ether) -> Self {
        ether.0
    }
}

impl From<i32> for Ether {
    fn from(value: i32) -> Self {
        Self(value.into())
    }
}

/// Block number.
#[derive(Debug, Clone, Copy)]
pub struct BlockNo(pub u64);

/// An onchain transaction which interacts with a smart contract.
#[derive(Debug, Clone)]
pub struct Interaction {
    pub target: Address,
    pub value: Ether,
    pub call_data: Vec<u8>,
}

/// An onchain transaction.
#[derive(Debug, Clone)]
pub struct Tx {
    pub from: Address,
    pub to: Address,
    pub value: Ether,
    pub input: Vec<u8>,
    pub access_list: AccessList,
}

impl Tx {
    pub fn set_access_list(self, access_list: AccessList) -> Self {
        Self {
            access_list,
            ..self
        }
    }
}
