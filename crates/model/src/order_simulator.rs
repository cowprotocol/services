use {
    alloy_primitives::{Address, B256, U256, map::B256Map},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::HashMap,
};

/// Tenderly API simulation request
/// https://docs.tenderly.co/reference/api#/operations/simulateTransaction#request-body
#[serde_as]
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct TenderlyRequest {
    /// ID of the network on which the simulation is being run.
    pub network_id: String,
    /// Number of the block to be used for the simulation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_number: Option<u64>,
    /// Index of the transaction within the block.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_index: Option<i64>,
    /// Address initiating the transaction.
    pub from: Address,
    /// The recipient address of the transaction.
    pub to: Address,
    /// Encoded contract method call data.
    #[serde_as(as = "serde_ext::Hex")]
    pub input: Vec<u8>,
    /// Amount of gas provided for the simulation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas: Option<u64>,
    /// String representation of a number that represents price of the gas in
    /// Wei.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gas_price: Option<u64>,
    /// Amount of Ether (in Wei) sent along with the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<U256>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub simulation_type: Option<SimulationType>,
    /// Flag indicating whether to save the simulation in dashboard UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    /// Flag indicating whether to save failed simulation in dashboard UI.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_if_fails: Option<bool>,
    /// Flag that enables returning the access list in a response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generate_access_list: Option<bool>,
    /// Overrides for a given contract.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state_objects: Option<HashMap<Address, StateObject>>,
    /// EIP-2930 access list used by the transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_list: Option<Vec<AccessListItem>>,
}

/// EIP-2930 access list used by the transaction.
/// https://docs.tenderly.co/reference/api#/operations/simulateTransaction#response-body:~:text=0x-,access_list,-array%20or%20null
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct AccessListItem {
    /// Accessed address
    pub address: Address,
    /// Accessed storage keys
    #[serde(default)]
    pub storage_keys: Vec<B256>,
}

/// Overrides for a given contract. In this mapping, the key is the contract
/// address, and the value is an object that contains overrides of nonce, code,
/// balance, or state. https://docs.tenderly.co/reference/api#/operations/simulateTransaction#response-body:~:text=null%2C%22uncles%22%3Anull%7D-,state_objects,-dictionary%5Bstring%2C%20object
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

/// Opt for quick, abi, or full simulation API mode.
/// full (default): Detailed decoded output — call trace, function
/// inputs/outputs, state diffs, and logs with Solidity types.
///
/// quick: Raw,
/// minimal output only. Fastest option; no decoding.
///
/// abi: Decoded function
/// inputs/outputs and logs, but no state diffs. Middle ground between quick and
/// full.
///
/// https://docs.tenderly.co/reference/api#/operations/simulateTransaction#response-body:~:text=true-,simulation_type,-string
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimulationType {
    Full,
    Quick,
    Abi,
}

/// The result of Order simulation, contains the error (if any)
/// and full Tenderly API request that can be used to resimulate
/// and debug using Tenderly
#[derive(Clone, Deserialize, Serialize, Debug)]
pub struct OrderSimulation {
    pub tenderly_request: TenderlyRequest,
    pub error: Option<String>,
}
