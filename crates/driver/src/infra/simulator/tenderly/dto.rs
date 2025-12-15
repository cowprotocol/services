//! Data transfer objects for interacting with the Tenderly API.

use {
    crate::{domain::eth, util::serialize},
    ethrpc::alloy::conversions::{IntoAlloy, IntoLegacy},
    itertools::Itertools,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Request {
    pub network_id: String,
    pub from: eth::Address,
    pub to: eth::Address,
    #[serde_as(as = "serialize::Hex")]
    pub input: Vec<u8>,
    pub value: eth::U256,
    pub save: bool,
    pub save_if_fails: bool,
    pub generate_access_list: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<AccessList>,
    pub gas_price: u64,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    pub transaction: Transaction,
    pub generated_access_list: Option<AccessList>,
    pub simulation: Simulation,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    pub status: bool,
    pub gas_used: u64,
}

/// Tenderly requires access lists to be serialized with `snake_case` instead
/// of the standard `camelCase`.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AccessList(Vec<AccessListItem>);

#[derive(Debug, Deserialize, Serialize)]
struct AccessListItem {
    address: eth::Address,
    #[serde(default)]
    storage_keys: Vec<eth::B256>,
}

impl From<eth::AccessList> for AccessList {
    fn from(value: eth::AccessList) -> Self {
        Self(
            web3::types::AccessList::from(value)
                .into_iter()
                .map(|item| AccessListItem {
                    address: item.address.into_alloy(),
                    storage_keys: item
                        .storage_keys
                        .into_iter()
                        .map(IntoAlloy::into_alloy)
                        .collect(),
                })
                .collect(),
        )
    }
}

impl From<AccessList> for eth::AccessList {
    fn from(value: AccessList) -> Self {
        value
            .0
            .into_iter()
            .map(|item| web3::types::AccessListItem {
                address: item.address.into_legacy(),
                storage_keys: item
                    .storage_keys
                    .into_iter()
                    .map(IntoLegacy::into_legacy)
                    .collect(),
            })
            .collect_vec()
            .into()
    }
}

#[derive(Debug, Deserialize)]
pub struct Simulation {
    pub id: String,
}
