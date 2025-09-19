//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    crate::price_estimation::trade_verifier::balance_overrides::BalanceOverriding,
    alloy::{dyn_abi::SolType, sol_types::sol_data},
    anyhow::{Context, Result},
    contracts::{ERC1271SignatureValidator, errors::EthcontractErrorType},
    ethcontract::{Bytes, state_overrides::StateOverrides},
    ethrpc::{Web3, alloy::conversions::IntoLegacy},
    primitive_types::{H160, U256},
    std::sync::Arc,
    tracing::instrument,
};

pub struct Validator {
    signatures: contracts::support::Signatures,
    settlement: contracts::GPv2Settlement,
    vault_relayer: H160,
    web3: Web3,
    balance_overrider: Arc<dyn BalanceOverriding>,
}

impl Validator {
    /// The result returned from `isValidSignature` if the signature is correct
    const IS_VALID_SIGNATURE_MAGIC_BYTES: &'static str = "1626ba7e";

    pub fn new(
        web3: &Web3,
        settlement: contracts::GPv2Settlement,
        signatures: contracts::support::Signatures,
        vault_relayer: H160,
        balance_overrider: Arc<dyn BalanceOverriding>,
    ) -> Self {
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "signatureValidation".into());
        Self {
            signatures,
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
        let contract = ERC1271SignatureValidator::at(&self.web3, check.signer);
        let magic_bytes = contract
            .methods()
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature.clone()))
            .call()
            .await
            .map(|value| hex::encode(value.0))?;

        if magic_bytes != Self::IS_VALID_SIGNATURE_MAGIC_BYTES {
            return Err(SignatureValidationError::Invalid);
        }

        Ok(())
    }

    #[instrument(skip_all, fields(interactions_len = check.interactions.len()))]
    async fn simulate(
        &self,
        check: SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        let overrides: StateOverrides = match check.balance_override {
            Some(overrides) => self
                .balance_overrider
                .state_override(overrides)
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
        let validate_call = self.signatures.methods().validate(
            (self.settlement.address(), self.vault_relayer),
            check.signer,
            Bytes(check.hash),
            Bytes(check.signature),
            check
                .interactions
                .into_iter()
                .map(|i| (i.target, i.value, Bytes(i.call_data)))
                .collect(),
        );
        let response_bytes = self
            .settlement
            .simulate_delegatecall(
                self.signatures.address(),
                Bytes(validate_call.tx.data.unwrap_or_default().0),
            )
            .from(crate::SIMULATION_ACCOUNT.clone())
            .call_with_state_overrides(&overrides)
            .await?;

        let gas_used = <sol_data::Uint<256>>::abi_decode(&response_bytes.0)
            .context("could not decode signature check result")?
            .into_legacy();

        Ok(Simulation { gas_used })
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Validator {
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

impl From<ethcontract::errors::MethodError> for SignatureValidationError {
    fn from(err: ethcontract::errors::MethodError) -> Self {
        match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Contract => Self::Invalid,
            _ => Self::Other(err.into()),
        }
    }
}
