use contracts::ERC1271SignatureValidator;
use ethcontract::{errors::MethodError, Bytes};
use futures::future;
use hex_literal::hex;
use primitive_types::H160;
use shared::{ethcontract_error::EthcontractErrorType, Web3};
use thiserror::Error;

/// Structure used to represent a signature.
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
    /// The Magical value as defined by EIP-1271
    pub const MAGICAL_VALUE: [u8; 4] = hex!("1626ba7e");

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
        match instance
            .is_valid_signature(Bytes(check.hash), Bytes(check.signature))
            .call()
            .await
        {
            Ok(Bytes(value)) if value == Self::MAGICAL_VALUE => Ok(()),
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

    async fn validate_signatures(
        &self,
        checks: Vec<SignatureCheck>,
    ) -> Vec<Result<(), SignatureValidationError>> {
        // TODO(nlordell): Use batch calls!
        future::join_all(
            checks
                .into_iter()
                .map(|check| self.validate_signature(check)),
        )
        .await
    }
}
