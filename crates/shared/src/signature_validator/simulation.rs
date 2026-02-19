//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    crate::price_estimation::trade_verifier::balance_overrides::BalanceOverriding,
    alloy::{
        dyn_abi::SolType,
        primitives::{Address, U256},
        rpc::types::state::StateOverride,
        sol_types::{SolCall, sol_data},
        transports::RpcError,
    },
    anyhow::{Context, Result},
    contracts::alloy::{
        ERC1271SignatureValidator::ERC1271SignatureValidator,
        GPv2Settlement,
        support::Signatures,
    },
    ethrpc::{Web3, alloy::ProviderLabelingExt},
    std::sync::Arc,
    tracing::instrument,
};

pub struct Validator {
    signatures_address: Address,
    settlement: GPv2Settlement::Instance,
    vault_relayer: Address,
    web3: Web3,
    balance_overrider: Arc<dyn BalanceOverriding>,
}

impl Validator {
    /// The result returned from `isValidSignature` if the signature is correct
    const IS_VALID_SIGNATURE_MAGIC_BYTES: &'static str = "1626ba7e";

    pub fn new(
        web3: &Web3,
        settlement: GPv2Settlement::Instance,
        signatures_address: Address,
        vault_relayer: Address,
        balance_overrider: Arc<dyn BalanceOverriding>,
    ) -> Self {
        let web3 = web3.labeled("signatureValidation");
        Self {
            signatures_address,
            settlement,
            vault_relayer,
            web3: web3.clone(),
            balance_overrider,
        }
    }

    /// Simulate isValidSignature for the cases in which the order does not
    /// require a "setup" in the form of pre-interactions or a flashloan.
    async fn simulate_without_setup(
        &self,
        check: SignatureCheck,
    ) -> Result<(), SignatureValidationError> {
        // Since there are no interactions (no dynamic conditions / complex pre-state
        // change), the order's validity can be directly determined by whether
        // the signature matches the expected hash of the order data, checked
        // with isValidSignature method called on the owner's contract
        let contract = ERC1271SignatureValidator::new(check.signer, &self.web3.provider);
        let magic_bytes = contract
            .isValidSignature(check.hash.into(), check.signature.clone().into())
            .call()
            .await
            .map(|value| const_hex::encode(value.0))
            .map_err(|err| match err {
                alloy::contract::Error::TransportError(RpcError::ErrorResp(err)) => {
                    tracing::error!(?err, "failed to call isValidSignature");
                    SignatureValidationError::Invalid
                }
                err => SignatureValidationError::Other(err.into()),
            })?;

        if magic_bytes != Self::IS_VALID_SIGNATURE_MAGIC_BYTES {
            return Err(SignatureValidationError::Invalid);
        }

        Ok(())
    }

    /// Simulates the signature validation setting balance overrides and
    /// pre-interactions; returning the gas used for the signature validation
    /// only.
    ///
    /// These are required as they may interact with the signature, for example,
    /// adding composable CoW orders.
    #[instrument(skip_all, fields(interactions_len = check.interactions.len()))]
    async fn simulate(
        &self,
        check: SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        let overrides: StateOverride = match &check.balance_override {
            Some(overrides) => self
                .balance_overrider
                .state_override(overrides.clone())
                .await
                .into_iter()
                .collect(),
            None => Default::default(),
        };
        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let validate_call = Signatures::Signatures::validateCall {
            contracts: Signatures::Signatures::Contracts {
                settlement: *self.settlement.address(),
                vaultRelayer: self.vault_relayer,
            },
            signer: check.signer,
            order: check.hash.into(),
            signature: check.signature.clone().into(),
            interactions: check
                .interactions
                .iter()
                .map(|i| Signatures::Signatures::Interaction {
                    target: i.target,
                    value: i.value,
                    callData: i.call_data.clone().into(),
                })
                .collect(),
        };
        let simulation = self
            .settlement
            .simulateDelegatecall(self.signatures_address, validate_call.abi_encode().into())
            .state(overrides.clone())
            .from(*crate::SIMULATION_ACCOUNT);

        let result = simulation.clone().call().await;

        let response_bytes = result
            .inspect_err(|err| {
                tracing::debug!(
                    ?simulation,
                    ?check,
                    ?overrides,
                    ?err,
                    "signature verification failed"
                )
            })
            .map_err(|_| SignatureValidationError::Invalid)?;

        let gas_used = <sol_data::Uint<256>>::abi_decode(&response_bytes.0).with_context(|| {
            format!(
                "could not decode signature check result: {}",
                const_hex::encode(&response_bytes.0)
            )
        })?;

        Ok(Simulation { gas_used })
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Validator {
    /// Validates a signature, setting up state for the simulation if needed.
    #[instrument(skip_all)]
    async fn validate_signature(
        &self,
        check: SignatureCheck,
    ) -> Result<(), SignatureValidationError> {
        if check.requires_setup() {
            self.simulate(check).await.map(|_| ())
        } else {
            self.simulate_without_setup(check).await
        }
    }

    async fn validate_signature_and_get_additional_gas(
        &self,
        check: SignatureCheck,
    ) -> Result<u64, SignatureValidationError> {
        Ok(self
            .simulate(check)
            .await?
            .gas_used
            .try_into()
            .unwrap_or(u64::MAX))
    }
}

#[derive(Debug)]
struct Simulation {
    gas_used: U256,
}
