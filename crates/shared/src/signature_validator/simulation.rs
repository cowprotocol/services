//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    crate::code_simulation::{CodeSimulating, SimulationError},
    anyhow::{Context, Result},
    ethcontract::{common::abi::Token, tokens::Tokenize, Bytes},
    ethrpc::extensions::StateOverride,
    futures::future,
    maplit::hashmap,
    primitive_types::{H160, U256},
    std::sync::Arc,
    web3::types::CallRequest,
};

pub struct Validator {
    simulator: Arc<dyn CodeSimulating>,
    settlement: H160,
    vault_relayer: H160,
}

impl Validator {
    pub fn new(simulator: Arc<dyn CodeSimulating>, settlement: H160, vault_relayer: H160) -> Self {
        Self {
            simulator,
            settlement,
            vault_relayer,
        }
    }

    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Verification, SignatureValidationError> {
        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let signatures =
            contracts::dummy_contract!(contracts::support::Signatures, self.settlement);
        let tx = signatures
            .methods()
            .validate(
                (self.settlement, self.vault_relayer),
                check.signer,
                Bytes(check.hash),
                Bytes(check.signature.clone()),
                check
                    .interactions
                    .iter()
                    .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                    .collect(),
            )
            .tx;

        let call = CallRequest {
            to: tx.to,
            data: tx.data,
            ..Default::default()
        };
        let overrides = hashmap! {
            signatures.address() => StateOverride {
                code: Some(contracts::deployed_bytecode!(contracts::support::Signatures)),
                ..Default::default()
            },
        };

        let output = self.simulator.simulate(call, overrides).await?;
        let simulation = Simulation::decode(&output)?;

        tracing::trace!(?check, ?simulation, "simulated signatures");
        match simulation.result {
            SimulationResult::Ok => Ok(simulation.verification),
            result => {
                tracing::debug!(?result, "invalid signature");
                Err(SignatureValidationError::Invalid)
            }
        }
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Validator {
    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        future::join_all(checks.into_iter().map(|check| async move {
            self.simulate(&check).await?;
            Ok(())
        }))
        .await
    }

    async fn validate_signature_and_get_additional_gas(
        &self,
        check: SignatureCheck,
    ) -> Result<u64, SignatureValidationError> {
        Ok(self
            .simulate(&check)
            .await?
            .gas_used
            .try_into()
            .unwrap_or(u64::MAX))
    }
}

#[derive(Debug)]
struct Simulation {
    result: SimulationResult,
    verification: Verification,
}

#[derive(Debug)]
struct Verification {
    gas_used: U256,
}

#[derive(Debug)]
enum SimulationResult {
    Ok,
    Invalid,
    Revert,
}

impl Simulation {
    fn decode(output: &[u8]) -> Result<Self> {
        let function = contracts::support::Signatures::raw_contract()
            .abi
            .function("balance")
            .unwrap();
        let tokens = function.decode_output(output).context("decode")?;
        let (result, gas_used) = Tokenize::from_token(Token::Tuple(tokens))?;

        Ok(Self {
            result: SimulationResult::decode(result)?,
            verification: Verification { gas_used },
        })
    }
}

impl SimulationResult {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Ok),
            1 => Ok(Self::Invalid),
            2 => Ok(Self::Revert),
            _ => anyhow::bail!("invalid simulation result variant {value}"),
        }
    }
}

impl From<SimulationError> for SignatureValidationError {
    fn from(err: SimulationError) -> Self {
        Self::Other(err.into())
    }
}
