use {
    crate::{
        ethcontract_error::EthcontractErrorType,
        ethrpc::{Web3, MAX_BATCH_SIZE},
    },
    contracts::ERC1271SignatureValidator,
    ethcontract::{
        batch::CallBatch,
        errors::{ExecutionError, MethodError},
        Bytes,
    },
    futures::future,
    hex_literal::hex,
    primitive_types::H160,
    thiserror::Error,
};

const TRANSACTION_INITIALIZATION_GAS_AMOUNT: u64 = 21_000u64;

/// Structure used to represent a signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureCheck {
    pub signer: H160,
    pub hash: [u8; 32],
    pub signature: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum SignatureValidationError {
    /// The signature is invalid.
    ///
    /// Either the calling contract reverted or did not return the magic value.
    #[error("invalid signature")]
    Invalid,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[mockall::automock]
#[async_trait::async_trait]
/// <https://eips.ethereum.org/EIPS/eip-1271>
pub trait SignatureValidating: Send + Sync {
    async fn validate_signature(
        &self,
        check: SignatureCheck,
    ) -> Result<(), SignatureValidationError>;

    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>>;

    /// Validates the signature and returns the `eth_estimateGas` of the
    /// isValidSignature call minus the tx initation gas amount of 21k.
    async fn validate_signature_and_get_additional_gas(
        &self,
        check: SignatureCheck,
    ) -> Result<u64, SignatureValidationError>;
}

pub struct Web3SignatureValidator {
    web3: Web3,
}

impl Web3SignatureValidator {
    pub fn new(web3: Web3) -> Self {
        Self { web3 }
    }
}

#[async_trait::async_trait]
impl SignatureValidating for Web3SignatureValidator {
    async fn validate_signature(
        &self,
        check: SignatureCheck,
    ) -> Result<(), SignatureValidationError> {
        let instance = ERC1271SignatureValidator::at(&self.web3, check.signer);
        let result = instance
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature))
            .call()
            .await?;

        check_erc1271_result(result)
    }

    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        let mut batch = CallBatch::new(self.web3.transport().clone());
        let calls = checks
            .into_iter()
            .map(|check| {
                let instance = ERC1271SignatureValidator::at(&self.web3, check.signer);
                let call = instance
                    .is_valid_signature(Bytes(check.hash), Bytes(check.signature))
                    .batch_call(&mut batch);

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
        let instance = ERC1271SignatureValidator::at(&self.web3, check.signer);
        let check = instance.is_valid_signature(Bytes(check.hash), Bytes(check.signature.clone()));

        let (result, gas_estimate) =
            futures::join!(check.clone().call(), check.m.tx.estimate_gas());

        check_erc1271_result(result?)?;

        // Adjust the estimate we receive by the fixed transaction gas cost.
        // This is because this cost is not paid by an internal call, but by
        // the root transaction only.
        Ok(gas_estimate?.as_u64() - TRANSACTION_INITIALIZATION_GAS_AMOUNT)
    }
}

/// The Magical value as defined by EIP-1271
const MAGICAL_VALUE: [u8; 4] = hex!("1626ba7e");

pub fn check_erc1271_result(result: Bytes<[u8; 4]>) -> Result<(), SignatureValidationError> {
    if result.0 == MAGICAL_VALUE {
        Ok(())
    } else {
        Err(SignatureValidationError::Invalid)
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

impl From<ExecutionError> for SignatureValidationError {
    fn from(err: ExecutionError) -> Self {
        match EthcontractErrorType::classify(&err) {
            EthcontractErrorType::Contract => Self::Invalid,
            _ => Self::Other(err.into()),
        }
    }
}
