//! Abstraction for simulating calls with overrides.

use crate::{
    ethcontract_error::EthcontractErrorType,
    tenderly_api::{SimulationKind, SimulationRequest, TenderlyApi},
    transport::extensions::{EthExt as _, StateOverrides},
    Web3,
};
use anyhow::{Context as _, Result};
use ethcontract::errors::ExecutionError;
use std::sync::Arc;
use thiserror::Error;
use web3::types::{BlockNumber, CallRequest};

/// Simulate a call with state overrides.
#[mockall::automock]
#[async_trait::async_trait]
pub trait CodeSimulating: Send + Sync + 'static {
    async fn simulate(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
    ) -> Result<Vec<u8>, SimulationError>;
}

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

#[async_trait::async_trait]
impl CodeSimulating for Web3 {
    async fn simulate(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
    ) -> Result<Vec<u8>, SimulationError> {
        Ok(self
            .eth()
            .call_with_state_overrides(call, BlockNumber::Latest.into(), overrides)
            .await?
            .0)
    }
}

#[derive(Default)]
struct SaveConfiguration {
    on_success: bool,
    on_failure: bool,
}

pub struct TenderlyCodeSimulator {
    tenderly: Arc<dyn TenderlyApi>,
    network_id: String,
    save: SaveConfiguration,
}

impl TenderlyCodeSimulator {
    pub fn new(tenderly: Arc<dyn TenderlyApi>, network_id: impl ToString) -> Self {
        Self {
            tenderly,
            network_id: network_id.to_string(),
            save: SaveConfiguration::default(),
        }
    }

    /// Configure the Tenderly code simulator to save simulations.
    pub fn save(mut self, on_success: bool, on_failure: bool) -> Self {
        self.save = SaveConfiguration {
            on_success,
            on_failure,
        };
        self
    }
}

#[async_trait::async_trait]
impl CodeSimulating for TenderlyCodeSimulator {
    async fn simulate(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
    ) -> Result<Vec<u8>, SimulationError> {
        let result = self
            .tenderly
            .simulate(SimulationRequest {
                network_id: self.network_id.clone(),
                from: call.from.unwrap_or_default(),
                to: call.to.unwrap_or_default(),
                input: call.data.unwrap_or_default().0,
                gas: call.gas.map(|g| g.as_u64()),
                gas_price: call.gas_price.map(|p| p.as_u64()),
                value: call.value,
                simulation_kind: Some(SimulationKind::Quick),
                state_objects: Some(overrides),
                save: Some(self.save.on_success),
                save_if_fails: Some(self.save.on_failure),
                ..Default::default()
            })
            .await?;

        let trace = result
            .transaction
            .call_trace
            .into_iter()
            .next()
            .context("Tenderly simulation missing call trace")?;

        if let Some(err) = trace.error {
            return Err(SimulationError::Revert(Some(err)));
        }

        let output = trace
            .output
            .context("Tenderly simulation missing transaction output")?;

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        tenderly_api::TenderlyHttpApi,
        transport::{create_env_test_transport, extensions::StateOverride},
    };
    use maplit::hashmap;

    #[ignore]
    #[tokio::test]
    async fn can_simulate_contract_code() {
        let web3 = Web3::new(create_env_test_transport());
        let network_id = web3.net().version().await.unwrap();

        for simulator in [
            Arc::new(web3) as Arc<dyn CodeSimulating>,
            Arc::new(TenderlyCodeSimulator::new(
                TenderlyHttpApi::test_from_env(),
                network_id,
            )),
        ] {
            let address = addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");
            let output = simulator
                .simulate(
                    CallRequest {
                        to: Some(address),
                        ..Default::default()
                    },
                    hashmap! {
                        address => StateOverride {
                            // EVM program to just returns the Answer to Life,
                            // the Universe, and Everything.
                            code: Some(bytes!(
                                "60 2a
                                 60 00
                                 53
                                 60 01
                                 60 00
                                 f3"
                            )),
                            ..Default::default()
                        },
                    },
                )
                .await
                .unwrap();

            assert_eq!(output, [42]);
        }
    }

    #[ignore]
    #[tokio::test]
    async fn errors_on_reverts() {
        let web3 = Web3::new(create_env_test_transport());
        let network_id = web3.net().version().await.unwrap();

        for simulator in [
            Arc::new(web3) as Arc<dyn CodeSimulating>,
            Arc::new(TenderlyCodeSimulator::new(
                TenderlyHttpApi::test_from_env(),
                network_id,
            )),
        ] {
            let address = addr!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");
            let result = simulator
                .simulate(
                    CallRequest {
                        to: Some(address),
                        ..Default::default()
                    },
                    hashmap! {
                        address => StateOverride {
                            // EVM program revert with a message.
                            code: Some(bytes!(
                                "60 64
                                 80
                                 60 0b
                                 60 00
                                 39
                                 60 00
                                 fd

                                 08c379a0
                                 0000000000000000000000000000000000000000000000000000000000000020
                                 0000000000000000000000000000000000000000000000000000000000000004
                                 706f6f7000000000000000000000000000000000000000000000000000000000"
                            )),
                            ..Default::default()
                        },
                    },
                )
                .await;

            // Tenderly isn't extracting the revert bytes, so we just get some
            // general revert message, so we can't assert the message is as
            // expected.
            assert!(matches!(result, Err(SimulationError::Revert(Some(_)))));
        }
    }
}
