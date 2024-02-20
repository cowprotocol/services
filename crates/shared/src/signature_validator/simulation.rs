//! An `eth_call` simulation based ERC-1271 signature verification
//! implementation. This allows orders with ERC-1271 signatures to be used that
//! only get setup as a pre-hook (such as creating a Composable CoW order with a
//! Safe in a pre-interaction).

use {
    super::{SignatureCheck, SignatureValidating, SignatureValidationError},
    crate::ethcontract_error::EthcontractErrorType,
    anyhow::Result,
    ethcontract::Bytes,
    ethrpc::Web3,
    futures::future,
    primitive_types::{H160, U256},
};

pub struct Validator {
    signatures: contracts::support::Signatures,
    settlement: H160,
    vault_relayer: H160,
}

impl Validator {
    pub fn new(web3: &Web3, settlement: H160, vault_relayer: H160) -> Self {
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "signatureValidation".into());
        Self {
            signatures: contracts::support::Signatures::at(&web3, settlement),
            settlement,
            vault_relayer,
        }
    }

    async fn simulate(
        &self,
        check: &SignatureCheck,
    ) -> Result<Simulation, SignatureValidationError> {
        // We simulate the signature verification from the Settlement contract's
        // context. This allows us to check:
        // 1. How the pre-interactions would behave as part of the settlement
        // 2. Simulate the actual `isValidSignature` calls that would happen as part of
        //    a settlement
        let gas_used = contracts::storage_accessible::simulate(
            contracts::bytecode!(contracts::support::Signatures),
            self.signatures.methods().validate(
                (self.settlement, self.vault_relayer),
                check.signer,
                Bytes(check.hash),
                Bytes(check.signature.clone()),
                check
                    .interactions
                    .iter()
                    .map(|i| (i.target, i.value.into(), Bytes(i.call_data.clone())))
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
