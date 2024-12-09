//! Abstraction for simulating calls with overrides.

use {
    crate::tenderly_api::{SimulationKind, SimulationRequest, StateObject, TenderlyApi},
    anyhow::{ensure, Context as _, Result},
    contracts::errors::EthcontractErrorType,
    ethcontract::errors::ExecutionError,
    ethrpc::{
        extensions::{EthExt as _, StateOverride, StateOverrides},
        Web3,
    },
    std::sync::Arc,
    thiserror::Error,
    web3::types::{BlockNumber, CallRequest},
};

/// Simulate a call with state overrides.
#[async_trait::async_trait]
pub trait CodeSimulating: Send + Sync + 'static {
    async fn simulate(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
        block: Option<u64>,
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
        block: Option<u64>,
    ) -> Result<Vec<u8>, SimulationError> {
        let block = block
            .map(|b| BlockNumber::Number(b.into()))
            .unwrap_or(BlockNumber::Latest);
        Ok(self
            .eth()
            .call_with_state_overrides(call, block.into(), overrides)
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

    fn prepare_request(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
        block: Option<u64>,
    ) -> Result<SimulationRequest> {
        Ok(SimulationRequest {
            block_number: block,
            // By default, tenderly simulates on the top of the specified block, whereas regular
            // nodes simulate at the end of the specified block. This is to make
            // simulation results match in case critical state changed within the block.
            transaction_index: Some(-1),
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
            ..Default::default()
        })
    }

    pub fn log_simulation_command(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
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

#[async_trait::async_trait]
impl CodeSimulating for TenderlyCodeSimulator {
    async fn simulate(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
        block: Option<u64>,
    ) -> Result<Vec<u8>, SimulationError> {
        let result = self
            .tenderly
            .simulate(SimulationRequest {
                save: Some(self.save.on_success),
                save_if_fails: Some(self.save.on_failure),
                ..self.prepare_request(call, overrides, block)?
            })
            .await?;

        let saved = self.save.on_success && result.transaction.status
            || self.save.on_failure && !result.transaction.status;
        if saved {
            tracing::debug!(
                url =% self.tenderly.simulation_url(&result.simulation.id),
                "saved simulation"
            );
        }

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
            storage: value.state_diff,
        })
    }
}

/// A code simulator that uses Web3 in the general case, but will create and
/// save failed simulations on Tenderly to facilitate debugging.
pub struct Web3ThenTenderly {
    web3: Web3,
    tenderly: Arc<TenderlyCodeSimulator>,
}

impl Web3ThenTenderly {
    pub fn new(web3: Web3, tenderly: TenderlyCodeSimulator) -> Self {
        Self {
            web3,
            tenderly: Arc::new(tenderly.save(true, true)),
        }
    }
}

#[async_trait::async_trait]
impl CodeSimulating for Web3ThenTenderly {
    async fn simulate(
        &self,
        call: CallRequest,
        overrides: StateOverrides,
        block: Option<u64>,
    ) -> Result<Vec<u8>, SimulationError> {
        let result = self
            .web3
            .simulate(call.clone(), overrides.clone(), block)
            .await;

        if let Err(err) = self.tenderly.log_simulation_command(call, overrides, block) {
            tracing::debug!(?err, "could not log tenderly simulation command");
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{ethrpc::create_env_test_transport, tenderly_api::TenderlyHttpApi},
        ethcontract::H256,
        hex_literal::hex,
        maplit::hashmap,
        std::time::Duration,
    };

    async fn test_simulators() -> Vec<Arc<dyn CodeSimulating>> {
        let web3 = Web3::new(create_env_test_transport());
        let network_id = web3.eth().chain_id().await.unwrap().to_string();

        vec![
            Arc::new(web3.clone()),
            Arc::new(TenderlyCodeSimulator::new(
                TenderlyHttpApi::test_from_env(),
                network_id.clone(),
            )),
            Arc::new(Web3ThenTenderly::new(
                web3,
                TenderlyCodeSimulator::new(TenderlyHttpApi::test_from_env(), network_id),
            )),
        ]
    }

    #[ignore]
    #[tokio::test]
    async fn can_simulate_contract_code() {
        for simulator in test_simulators().await {
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
                    None,
                )
                .await
                .unwrap();

            assert_eq!(output, [42]);
        }

        // Make sure to wait for background futures - `tokio::test` does not
        // seem to do this.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    #[ignore]
    #[tokio::test]
    async fn errors_on_reverts() {
        for simulator in test_simulators().await {
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
                    None,
                )
                .await;

            // Tenderly isn't extracting the revert bytes, so we just get some
            // general revert message, so we can't assert the message is as
            // expected.
            assert!(matches!(result, Err(SimulationError::Revert(Some(_)))));
        }

        // Make sure to wait for background futures - `tokio::test` does not
        // seem to do this.
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    #[tokio::test]
    #[ignore]
    async fn tenderly_state_override_conversion() {
        let web3 = Web3::new(create_env_test_transport());
        let network_id = web3.eth().chain_id().await.unwrap().to_string();
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
                            H256(balance_slot) =>
                                H256(hex!("0000000000000000000000000000000000000000000000000000000000000001")),
                        }),
                        ..Default::default()
                    },
                },
                None,
            )
            .await;

        assert!(result.is_ok());
    }
}
