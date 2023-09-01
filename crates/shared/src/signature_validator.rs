mod simulation;
mod web3;

use {
    ethcontract::Bytes,
    hex_literal::hex,
    model::interaction::InteractionData,
    primitive_types::H160,
    thiserror::Error,
};

pub use self::{simulation::Validator as SimulationValidator, web3::Web3SignatureValidator};

/// Structure used to represent a signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureCheck {
    pub signer: H160,
    pub hash: [u8; 32],
    pub signature: Vec<u8>,
    pub interactions: Vec<InteractionData>,
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

/// The Magical value as defined by EIP-1271
const MAGICAL_VALUE: [u8; 4] = hex!("1626ba7e");

pub fn check_erc1271_result(result: Bytes<[u8; 4]>) -> Result<(), SignatureValidationError> {
    if result.0 == MAGICAL_VALUE {
        Ok(())
    } else {
        Err(SignatureValidationError::Invalid)
    }
}
