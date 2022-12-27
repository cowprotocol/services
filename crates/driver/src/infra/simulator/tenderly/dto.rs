//! Data transfer objects for interacting with the Tenderly API.

use {
    crate::util::serialize,
    primitive_types::{H160, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Serialize)]
pub struct Request {
    pub network_id: String,
    pub from: H160,
    pub to: H160,
    #[serde_as(as = "serialize::Hex")]
    pub input: Vec<u8>,
    pub value: U256,
    pub save: bool,
    pub save_if_fails: bool,
    pub generate_access_list: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<web3::types::AccessList>,
}

#[derive(Debug, Deserialize)]
pub struct Response {
    transaction: Transaction,
    generated_access_list: Option<web3::types::AccessList>,
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
