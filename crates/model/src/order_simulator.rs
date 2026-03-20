use {
    alloy_primitives::{Address, B256, U256, map::B256Map},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::HashMap,
};

#[serde_as]
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct TenderlyRequest {
    pub network_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<i64>,
    pub from: Address,
    pub to: Address,
    #[serde_as(as = "serde_ext::Hex")]
    pub input: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulation_kind: Option<SimulationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_if_fails: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_access_list: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_objects: Option<HashMap<Address, StateObject>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<AccessListItem>>,
}

// Tenderly responds with
// snake_case fields and tenderly storage_keys field does not exist
// if empty (it should be empty Vec instead)
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccessListItem {
    /// Accessed address
    pub address: Address,
    /// Accessed storage keys
    #[serde(default)]
    pub storage_keys: Vec<B256>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct StateObject {
    /// Fake balance to set for the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<alloy_primitives::U256>,

    /// Fake EVM bytecode to inject into the account before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<alloy_primitives::Bytes>,

    /// Fake key-value mapping to override **individual** slots in the account
    /// storage before executing the call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<B256Map<B256>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimulationKind {
    Full,
    Quick,
}

#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct OrderSimulation {
    pub tenderly_request: TenderlyRequest,
    pub error: Option<String>,
}
