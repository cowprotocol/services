use contracts::ERC1271SignatureValidator;
use ethcontract::{batch::CallBatch, errors::MethodError, Bytes};
use futures::future;
use hex_literal::hex;
use primitive_types::H160;
use shared::{ethcontract_error::EthcontractErrorType, Web3};
use thiserror::Error;

/// Structure used to represent a signature.
#[derive(Clone, Debug, PartialEq)]
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
    /// A generic Web3 method error occured.
    #[error(transparent)]
    Other(#[from] MethodError),
}

#[cfg_attr(test, mockall::automock)]
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

        future::join_all(calls).await
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
        Err(err) => Err(SignatureValidationError::Other(err)),
    }
}
