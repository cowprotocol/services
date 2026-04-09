use {
    alloy_primitives::{Address, B256},
    derive_more::{From, Into},
    std::collections::{HashMap, HashSet},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Into, From)]
pub struct StorageKey(pub B256);

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

impl AccessList {
    /// Merge two access lists together.
    pub fn merge(mut self, other: Self) -> Self {
        for (address, storage_keys) in other.0.into_iter() {
            self.0.entry(address).or_default().extend(storage_keys);
        }
        self
    }
}

impl IntoIterator for AccessList {
    type IntoIter = std::collections::hash_map::IntoIter<Address, HashSet<StorageKey>>;
    type Item = (Address, HashSet<StorageKey>);

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<I> FromIterator<(Address, I)> for AccessList
where
    I: IntoIterator<Item = B256>,
{
    fn from_iter<T: IntoIterator<Item = (Address, I)>>(iter: T) -> Self {
        Self(
            iter.into_iter()
                .map(|(address, i)| {
                    (
                        address,
                        i.into_iter().map(StorageKey).collect::<HashSet<_>>(),
                    )
                })
                .collect(),
        )
    }
}

impl From<alloy_eips::eip2930::AccessList> for AccessList {
    fn from(value: alloy_eips::eip2930::AccessList) -> Self {
        Self(
            value
                .0
                .into_iter()
                .map(|item| {
                    (
                        item.address,
                        item.storage_keys
                            .into_iter()
                            .map(StorageKey)
                            .collect::<HashSet<_>>(),
                    )
                })
                .collect(),
        )
    }
}

impl From<AccessList> for alloy_eips::eip2930::AccessList {
    fn from(value: AccessList) -> Self {
        Self(
            value
                .into_iter()
                .map(
                    |(address, storage_keys)| alloy_eips::eip2930::AccessListItem {
                        address,
                        storage_keys: storage_keys.into_iter().map(|k| k.0).collect(),
                    },
                )
                .collect(),
        )
    }
}
