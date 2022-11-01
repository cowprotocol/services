//! Abstraction for simulating calls with overrides.

use crate::{
    ethcontract_error::EthcontractErrorType,
    ethrpc::{
        extensions::{EthExt as _, StateOverride, StateOverrides},
        Web3,
    },
    tenderly_api::{SimulationKind, SimulationRequest, StateObject, TenderlyApi},
};
use anyhow::{ensure, Context as _, Result};
use ethcontract::{errors::ExecutionError, H256};
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
                state_objects: Some(
                    overrides
                        .into_iter()
                        .map(|(key, value)| Ok((key, value.try_into()?)))
                        .collect::<Result<_>>()?,
                ),
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
            storage: value.state_diff.map(|state_diff| {
                state_diff
                    .into_iter()
                    .map(|(key, uint)| {
                        let mut value = H256::default();
                        uint.to_big_endian(&mut value.0);
                        (key, value)
                    })
                    .collect()
            }),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ethrpc::create_env_test_transport, tenderly_api::TenderlyHttpApi};
    use hex_literal::hex;
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

    #[tokio::test]
    #[ignore]
    async fn tenderly_state_override_conversion() {
        let web3 = Web3::new(create_env_test_transport());
        let network_id = web3.net().version().await.unwrap();
        let tenderly = TenderlyCodeSimulator::new(TenderlyHttpApi::test_from_env(), network_id)
            .save(true, true);

        let balance_slot = hex!("73bd07999de6b89204eec9e8f51c59f3b7dc1c94710ccdaf6f1f8e10e6391b56");
        let result = tenderly
            .simulate(
                CallRequest {
                    to: Some(addr!("D533a949740bb3306d119CC777fa900bA034cd52")),
                    from: Some(addr!("4242424242424242424242424242424242424242")),
                    data: Some(bytes!(
                        "a9059cbb
                         0000000000000000000000001337133713371337133713371337133713371337
                         0000000000000000000000000000000000000000000000000000000000000001"
                    )),
                    ..Default::default()
                },
                hashmap! {
                    addr!("D533a949740bb3306d119CC777fa900bA034cd52") => StateOverride {
                        state_diff: Some(hashmap! {
                            H256(balance_slot) => 1.into()
                        }),
                        ..Default::default()
                    },
                },
            )
            .await;

        assert!(result.is_ok());
    }
}
