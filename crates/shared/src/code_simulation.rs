//! Abstraction for simulating calls with overrides.

use crate::{
    tenderly_api::{SimulationKind, SimulationRequest, TenderlyApi},
    transport::extensions::{EthExt as _, StateOverrides},
    Web3,
};
use anyhow::{bail, Context as _, Result};
use std::sync::Arc;
use web3::types::{BlockNumber, CallRequest};

/// Simulate a call with state overrides.
#[async_trait::async_trait]
pub trait CodeSimulating: Send + Sync + 'static {
    async fn simulate(&self, call: CallRequest, overrides: StateOverrides) -> Result<Vec<u8>>;
}

#[async_trait::async_trait]
impl CodeSimulating for Web3 {
    async fn simulate(&self, call: CallRequest, overrides: StateOverrides) -> Result<Vec<u8>> {
        Ok(self
            .eth()
            .call_with_state_overrides(call, BlockNumber::Latest.into(), overrides)
            .await?
            .0)
    }
}

pub struct TenderlyCodeSimlator {
    tenderly: Arc<dyn TenderlyApi>,
    network_id: String,
    save: bool,
    save_if_fails: bool,
}

impl TenderlyCodeSimlator {
    pub fn new(tenderly: Arc<dyn TenderlyApi>, network_id: impl ToString) -> Self {
        Self {
            tenderly,
            network_id: network_id.to_string(),
            save: false,
            save_if_fails: false,
        }
    }

    /// Configure the Tenderly code simulator to save simulations.
    pub fn save(mut self, on_success: bool, on_failure: bool) -> Self {
        self.save = on_success;
        self.save_if_fails = on_failure;
        self
    }
}

#[async_trait::async_trait]
impl CodeSimulating for TenderlyCodeSimlator {
    async fn simulate(&self, call: CallRequest, overrides: StateOverrides) -> Result<Vec<u8>> {
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
                save: Some(self.save),
                save_if_fails: Some(self.save_if_fails),
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
            bail!("Tenderly simulation error: {err}");
        }

        trace
            .output
            .context("Tenderly simulation missing transaction output")
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
            Arc::new(TenderlyCodeSimlator::new(
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
            Arc::new(TenderlyCodeSimlator::new(
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

            assert!(result.is_err());
        }
    }
}
