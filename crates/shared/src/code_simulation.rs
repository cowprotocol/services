//! Abstraction for simulating calls with overrides.

use {
    crate::tenderly_api::{SimulationKind, SimulationRequest, StateObject, TenderlyApi},
    alloy::{
        contract::CallBuilder,
        primitives::TxKind,
        rpc::types::state::StateOverride as AlloyStateOverride,
    },
    anyhow::{Result, ensure},
    contracts::errors::EthcontractErrorType,
    ethcontract::{errors::ExecutionError, state_overrides::StateOverride},
    ethrpc::{AlloyProvider, alloy::conversions::IntoLegacy},
    std::sync::Arc,
    thiserror::Error,
};

#[derive(Debug, Error)]
pub enum SimulationError {
    #[error("simulation reverted {0:?}")]
    Revert(Option<String>),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Clone for SimulationError {
    fn clone(&self) -> Self {
        match self {
            Self::Revert(message) => Self::Revert(message.clone()),
            Self::Other(err) => Self::Other(crate::clone_anyhow_error(err)),
        }
    }
}

impl From<web3::Error> for SimulationError {
    fn from(err: web3::Error) -> Self {
        let err = ExecutionError::from(err);
        match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Node => Self::Other(err.into()),
            EthcontractErrorType::Contract => match err {
                ExecutionError::Revert(message) => Self::Revert(message),
                _ => Self::Revert(None),
            },
        }
    }
}

pub struct TenderlyCodeSimulator {
    tenderly: Arc<dyn TenderlyApi>,
    network_id: String,
}

impl TenderlyCodeSimulator {
    pub fn new(tenderly: Arc<dyn TenderlyApi>, network_id: impl ToString) -> Self {
        Self {
            tenderly,
            network_id: network_id.to_string(),
        }
    }

    fn prepare_request(
        &self,
        call: CallBuilder<AlloyProvider, ()>,
        overrides: AlloyStateOverride,
        block: Option<u64>,
    ) -> Result<SimulationRequest> {
        let tx = call.into_transaction_request();

        Ok(SimulationRequest {
            block_number: block,
            // By default, tenderly simulates on the top of the specified block, whereas regular
            // nodes simulate at the end of the specified block. This is to make
            // simulation results match in case critical state changed within the block.
            transaction_index: Some(-1),
            network_id: self.network_id.clone(),
            from: tx.from.map(IntoLegacy::into_legacy).unwrap_or_default(),
            to: tx
                .to
                .map(TxKind::into_to)
                .flatten()
                .map(IntoLegacy::into_legacy)
                .unwrap_or_default(),
            input: tx
                .input
                .into_input()
                .map(IntoLegacy::into_legacy)
                .unwrap_or_default()
                .0,
            gas: tx.gas,
            gas_price: tx
                .gas_price
                .map(TryInto::try_into)
                .map(|gas_price| gas_price.unwrap()),
            value: tx.value.map(IntoLegacy::into_legacy),
            simulation_kind: Some(SimulationKind::Quick),
            state_objects: Some(
                overrides
                    .into_legacy()
                    .into_iter()
                    .map(|(key, value)| Ok((key, value.try_into()?)))
                    .collect::<Result<_>>()?,
            ),
            ..Default::default()
        })
    }

    pub fn log_simulation_command(
        &self,
        call: CallBuilder<AlloyProvider, ()>,
        overrides: AlloyStateOverride,
        block: Option<u64>,
    ) -> Result<()> {
        let request = SimulationRequest {
            save: Some(true),
            save_if_fails: Some(true),
            ..self.prepare_request(call, overrides, block)?
        };
        self.tenderly.log(request)
    }
}

impl TryFrom<StateOverride> for StateObject {
    type Error = anyhow::Error;

    fn try_from(value: StateOverride) -> Result<Self, Self::Error> {
        ensure!(
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
