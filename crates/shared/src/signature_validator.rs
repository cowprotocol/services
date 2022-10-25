use crate::{ethcontract_error::EthcontractErrorType, transport::MAX_BATCH_SIZE, Web3};
use contracts::ERC1271SignatureValidator;
use ethcontract::{
    batch::CallBatch,
    errors::{ExecutionError, MethodError},
    Bytes,
};
use futures::future;
use hex_literal::hex;
use primitive_types::H160;
use thiserror::Error;

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
    /// A generic Web3 method error occurred.
    #[error(transparent)]
    Method(#[from] MethodError),
    /// An error occurred while estimating gas for isValidSignature
    #[error(transparent)]
    Execution(#[from] ExecutionError),
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
            .await;

        parse_is_valid_signature_result(result)
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

                async move { parse_is_valid_signature_result(call.await) }
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
        let is_valid_result = instance
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature.clone()))
            .call()
            .await;

        let is_valid_gas_estimate_with_tx_initiation = instance
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature))
            .m
            .tx
            .estimate_gas()
            .await
            .map_err(SignatureValidationError::Execution)?;

        // Since all gas amounts should be smaller the the blocksize of 15M,
        // the following operation should never panic
        let is_valid_gas_estimate = is_valid_gas_estimate_with_tx_initiation.as_u64()
            - TRANSACTION_INITIALIZATION_GAS_AMOUNT;

        parse_is_valid_signature_result(is_valid_result).map(|_| is_valid_gas_estimate)
    }
}

/// The Magical value as defined by EIP-1271
const MAGICAL_VALUE: [u8; 4] = hex!("1626ba7e");

fn parse_is_valid_signature_result(
    result: Result<Bytes<[u8; 4]>, MethodError>,
) -> Result<(), SignatureValidationError> {
    match result {
        Ok(Bytes(value)) if value == MAGICAL_VALUE => Ok(()),
        Ok(_) => Err(SignatureValidationError::Invalid),
        // Classify "contract" errors as invalid signatures instead of node
        // errors (which may be temporary). This can happen if there is ABI
        // compability issues or calling an EOA instead of a SC.
        Err(err) if EthcontractErrorType::classify(&err) == EthcontractErrorType::Contract => {
            Err(SignatureValidationError::Invalid)
        }
        Err(err) => Err(SignatureValidationError::Method(err)),
    }
}
