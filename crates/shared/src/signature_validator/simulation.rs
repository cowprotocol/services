//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    anyhow::Result,
    contracts::{errors::EthcontractErrorType, ERC1271SignatureValidator},
    ethcontract::Bytes,
    ethrpc::Web3,
    futures::future,
    primitive_types::{H160, U256},
    std::sync::LazyLock,
};

pub struct Validator {
    signatures: contracts::support::Signatures,
    settlement: H160,
    vault_relayer: H160,
    web3: Web3,
}

impl Validator {
    /// The result returned from `isValidSignature` if the signature is correct
    const IS_VALID_SIGNATURE_MAGIC_BYTES: &'static str = "1626ba7e";

    pub fn new(web3: &Web3, settlement: H160, vault_relayer: H160) -> Self {
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "signatureValidation".into());
        Self {
            signatures: contracts::support::Signatures::at(&web3, settlement),
            settlement,
            vault_relayer,
            web3: web3.clone(),
        }
    }

    /// Simulate isValidSignature for the cases in which the order does not have
    /// pre-interactions
    async fn simulate_without_pre_interactions(
        &self,
        check: &SignatureCheck,
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

    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        // memoize byte code to not hex-decode it on every call
        static BYTECODE: LazyLock<web3::types::Bytes> =
            LazyLock::new(|| contracts::bytecode!(contracts::support::Signatures));

        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let gas_used = contracts::storage_accessible::simulate(
            BYTECODE.clone(),
            self.signatures.methods().validate(
                (self.settlement, self.vault_relayer),
                check.signer,
                Bytes(check.hash),
                Bytes(check.signature.clone()),
                check
                    .interactions
                    .iter()
                    .map(|i| (i.target, i.value, Bytes(i.call_data.clone())))
                    .collect(),
            ),
        )
        .await?;

        let simulation = Simulation { gas_used };

        tracing::trace!(?check, ?simulation, "simulated signature");
        Ok(simulation)
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Validator {
    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        future::join_all(checks.into_iter().map(|check| async move {
            if check.interactions.is_empty() {
                self.simulate_without_pre_interactions(&check).await?;
            } else {
                self.simulate(&check).await?;
            }
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
