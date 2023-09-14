use {
    super::{check_erc1271_result, SignatureCheck, SignatureValidating, SignatureValidationError},
    crate::{
        ethcontract_error::EthcontractErrorType,
        ethrpc::{Web3, MAX_BATCH_SIZE},
    },
    contracts::ERC1271SignatureValidator,
    ethcontract::{
        batch::CallBatch,
        dyns::DynViewMethodBuilder,
        errors::{ExecutionError, MethodError},
        Bytes,
    },
    futures::future,
};

const TRANSACTION_INITIALIZATION_GAS_AMOUNT: u64 = 21_000u64;

pub struct Web3SignatureValidator {
    web3: Web3,
}

impl Web3SignatureValidator {
    pub fn new(web3: Web3) -> Self {
        Self { web3 }
    }
}

impl Web3SignatureValidator {
    /// Creates the `ethcontract` method call
    pub fn is_valid_signature(
        &self,
        check: SignatureCheck,
    ) -> DynViewMethodBuilder<Bytes<[u8; 4]>> {
        let instance = ERC1271SignatureValidator::at(&self.web3, check.signer);
        instance
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature))
            // Some signatures may internally trap, which will use up all
            // available gas. Setting a deterministic gas limit makes the gas
            // computation deterministic, and technically allows these
            // signatures to work within a settlement.
            // Note that this is the same gas limit that is used for simulations
            // in the `solver` crate.
            .gas(15_000_000_u128.into())
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Web3SignatureValidator {
    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        let mut batch = CallBatch::new(self.web3.transport().clone());
        let calls = checks
            .into_iter()
            .map(|check| {
                if !check.interactions.is_empty() {
                    tracing::warn!(
                        ?check,
                        "verifying ERC-1271 signatures with interactions is not fully supported"
                    );
                }

                let call = self.is_valid_signature(check).batch_call(&mut batch);
                async move { check_erc1271_result(call.await?) }
            })
            .collect::<Vec<_>>();

        batch.execute_all(MAX_BATCH_SIZE).await;
        future::join_all(calls).await
    }

    async fn validate_signature_and_get_additional_gas(
        &self,
        check: SignatureCheck,
    ) -> Result<u64, SignatureValidationError> {
        if !check.interactions.is_empty() {
            tracing::warn!(
                ?check,
                "verifying ERC-1271 signatures with interactions is not fully supported"
            );
        }

        let check = self.is_valid_signature(check);
        let (result, gas_estimate) =
            futures::join!(check.clone().call(), check.m.tx.estimate_gas());

        check_erc1271_result(result?)?;

        // Adjust the estimate we receive by the fixed transaction gas cost.
        // This is because this cost is not paid by an internal call, but by
        // the root transaction only.
        Ok(gas_estimate?.as_u64() - TRANSACTION_INITIALIZATION_GAS_AMOUNT)
    }
}

impl From<MethodError> for SignatureValidationError {
    fn from(err: MethodError) -> Self {
        // Classify "contract" errors as invalid signatures instead of node
        // errors (which may be temporary). This can happen if there is ABI
        // compability issues or calling an EOA instead of a SC.
        match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Contract => Self::Invalid,
            _ => Self::Other(err.into()),
        }
    }
}
