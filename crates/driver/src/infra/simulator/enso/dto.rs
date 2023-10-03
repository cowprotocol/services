//! Data transfer objects for interacting with the Enso Trade Simulator API.

use {
    crate::{domain::eth, util::serialize},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

#[serde_as]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    pub chain_id: u64,
    pub from: eth::H160,
    pub to: eth::H160,
    #[serde_as(as = "serialize::Hex")]
    pub data: Vec<u8>,
    pub value: eth::U256,
    pub gas_limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub gas_used: u64,
    pub block_number: u64,
    pub success: bool,
    pub exit_reason: String,
    #[serde_as(as = "serialize::Hex")]
    pub return_data: Vec<u8>,
}
