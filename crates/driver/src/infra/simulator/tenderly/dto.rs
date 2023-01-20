//! Data transfer objects for interacting with the Tenderly API.

use {
    crate::{domain::eth, util::serialize},
    itertools::Itertools,
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Request {
    pub network_id: String,
    pub from: eth::H160,
    pub to: eth::H160,
    #[serde_as(as = "serialize::Hex")]
    pub input: Vec<u8>,
    pub value: eth::U256,
    pub save: bool,
    pub save_if_fails: bool,
    pub generate_access_list: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<AccessList>,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    transaction: Transaction,
    generated_access_list: Option<AccessList>,
}

impl From<Response> for super::Simulation {
    fn from(res: Response) -> Self {
        Self {
            gas: res.transaction.gas_used.into(),
            access_list: res.generated_access_list.unwrap_or_default().into(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Transaction {
    gas_used: u64,
}

/// Tenderly requires access lists to be serialized with `snake_case` instead
/// of the standard `camelCase`.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AccessList(Vec<AccessListItem>);

#[derive(Debug, Deserialize, Serialize)]
struct AccessListItem {
    address: eth::H160,
    #[serde(default)]
    storage_keys: Vec<eth::H256>,
}

impl From<eth::AccessList> for AccessList {
    fn from(value: eth::AccessList) -> Self {
        Self(
            web3::types::AccessList::from(value)
                .into_iter()
                .map(|item| AccessListItem {
                    address: item.address,
                    storage_keys: item.storage_keys,
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
                address: item.address,
                storage_keys: item.storage_keys,
            })
            .collect_vec()
            .into()
    }
}
