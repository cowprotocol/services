use {
    alloy_primitives::{Address, B256, U256, map::B256Map},
    eth_domain_types as eth,
    model::order_simulator::{self, TenderlyRequest},
    serde::{Deserialize, Serialize},
    serde_with::serde_as,
    std::collections::HashMap,
};

/// Tenderly API simulation request
/// https://docs.tenderly.co/reference/api#/operations/simulateTransaction#request-body
#[serde_as]
#[derive(Clone, Deserialize, Serialize, Debug, Default)]
pub struct Request {
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
    pub access_list: Option<AccessList>,
}

impl From<Request> for TenderlyRequest {
    fn from(value: Request) -> Self {
        Self {
            network_id: value.network_id,
            block_number: value.block_number,
            transaction_index: value.transaction_index,
            from: value.from,
            to: value.to,
            input: value.input,
            gas: value.gas,
            gas_price: value.gas_price,
            value: value.value,
            simulation_type: value.simulation_type.map(|kind| match kind {
                SimulationType::Full => order_simulator::SimulationType::Full,
                SimulationType::Quick => order_simulator::SimulationType::Quick,
                SimulationType::Abi => order_simulator::SimulationType::Abi,
            }),
            save: value.save,
            save_if_fails: value.save_if_fails,
            generate_access_list: value.generate_access_list,
            state_objects: value.state_objects.map(|state_objects| {
                state_objects
                    .into_iter()
                    .map(|(key, state_object)| {
                        (
                            key,
                            order_simulator::StateObject {
                                balance: state_object.balance,
                                code: state_object.code,
                                storage: state_object.storage,
                            },
                        )
                    })
                    .collect()
            }),
            access_list: value.access_list.map(|access_list| {
                access_list
                    .0
                    .into_iter()
                    .map(|item| order_simulator::AccessListItem {
                        address: item.address,
                        storage_keys: item.storage_keys,
                    })
                    .collect()
            }),
        }
    }
}

impl From<TenderlyRequest> for Request {
    fn from(value: TenderlyRequest) -> Self {
        Self {
            network_id: value.network_id,
            block_number: value.block_number,
            transaction_index: value.transaction_index,
            from: value.from,
            to: value.to,
            input: value.input,
            gas: value.gas,
            gas_price: value.gas_price,
            value: value.value,
            simulation_type: value.simulation_type.map(|kind| match kind {
                order_simulator::SimulationType::Full => SimulationType::Full,
                order_simulator::SimulationType::Quick => SimulationType::Quick,
                order_simulator::SimulationType::Abi => SimulationType::Abi,
            }),
            save: value.save,
            save_if_fails: value.save_if_fails,
            generate_access_list: value.generate_access_list,
            state_objects: value.state_objects.map(|state_objects| {
                state_objects
                    .into_iter()
                    .map(|(key, state_object)| {
                        (
                            key,
                            StateObject {
                                balance: state_object.balance,
                                code: state_object.code,
                                storage: state_object.storage,
                            },
                        )
                    })
                    .collect()
            }),
            access_list: value.access_list.map(|access_list| {
                AccessList(
                    access_list
                        .into_iter()
                        .map(|item| AccessListItem {
                            address: item.address,
                            storage_keys: item.storage_keys,
                        })
                        .collect(),
                )
            }),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Response {
    pub transaction: Transaction,
    pub generated_access_list: Option<AccessList>,
    pub simulation: Simulation,
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

/// Tenderly requires access lists to be serialized with `snake_case` instead
/// of the standard `camelCase`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AccessList(Vec<AccessListItem>);

impl From<&alloy_rpc_types::AccessList> for AccessList {
    fn from(value: &alloy_rpc_types::AccessList) -> Self {
        AccessList(
            value
                .iter()
                .map(|item| AccessListItem {
                    address: item.address,
                    storage_keys: item.storage_keys.clone(),
                })
                .collect(),
        )
    }
}

impl From<AccessList> for alloy_rpc_types::AccessList {
    fn from(value: AccessList) -> Self {
        value
            .0
            .iter()
            .map(|item| alloy_rpc_types::AccessListItem {
                address: item.address,
                storage_keys: item.storage_keys.clone(),
            })
            .collect::<Vec<_>>()
            .into()
    }
}

impl From<eth::AccessList> for AccessList {
    fn from(value: eth::AccessList) -> Self {
        Self(
            value
                .into_iter()
                .map(|(address, storage_keys)| AccessListItem {
                    address,
                    storage_keys: storage_keys.into_iter().map(|k| k.0).collect(),
                })
                .collect(),
        )
    }
}

impl From<AccessList> for eth::AccessList {
    fn from(value: AccessList) -> Self {
        Self::from_iter(
            value
                .0
                .into_iter()
                .map(|item| (item.address, item.storage_keys)),
        )
    }
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

impl TryFrom<alloy_rpc_types::eth::state::AccountOverride> for StateObject {
    type Error = anyhow::Error;

    fn try_from(
        value: alloy_rpc_types::eth::state::AccountOverride,
    ) -> std::result::Result<Self, Self::Error> {
        anyhow::ensure!(
            value.nonce.is_none() && value.state.is_none(),
            "full state and nonce overrides not supported on Tenderly",
        );

        Ok(StateObject {
            balance: value.balance,
            code: value.code,
            storage: value.state_diff,
        })
    }
}
/// Opt for quick, abi, or full simulation API mode.
/// https://docs.tenderly.co/reference/api#/operations/simulateTransaction#response-body:~:text=true-,simulation_type,-string
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SimulationType {
    /// Detailed decoded output — call trace, function
    /// inputs/outputs, state diffs, and logs with Solidity types.
    Full,
    /// Raw, minimal output only. Fastest option; no decoding.
    Quick,
    /// Decoded function inputs/outputs and logs, but no state diffs. Middle
    /// ground between quick and full.
    Abi,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Transaction {
    pub status: bool,
    pub gas_used: u64,
    pub call_trace: Vec<CallTrace>,
}

#[serde_with::serde_as]
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CallTrace {
    #[serde(default)]
    #[serde_as(as = "Option<serde_ext::Hex>")]
    pub output: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Simulation {
    pub id: String,
}
