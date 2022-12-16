use {
    crate::util::serialize,
    primitive_types::{H160, H256, U256},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
};

// TODO Mapping into eth::Simulation
// TODO Mapping from eth::Tx

#[serde_as]
#[derive(Debug, Serialize)]
struct Request {
    network_id: String,
    from: H160,
    to: H160,
    #[serde_as(as = "serialize::Hex")]
    input: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<U256>,
    save: bool,
    save_if_fails: bool,
    generate_access_list: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    access_list: Option<Vec<AccessListItem>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessListItem {
    address: H160,
    storage_keys: Vec<H256>,
}

#[derive(Debug, Deserialize)]
struct Response {
    transaction: Transaction,
    generated_access_list: Option<Vec<AccessListItem>>,
}

#[derive(Debug, Deserialize)]
struct Transaction {
    gas_used: u64,
}
