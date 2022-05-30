use anyhow::Result;
use ethcontract::{batch::CallBatch, Bytes};
use primitive_types::H160;
use shared::Web3;

#[derive(Debug)]
pub enum IsValidSignatureError {
    ContractCallFailed,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
/// <https://eips.ethereum.org/EIPS/eip-1271>
pub trait SignatureValidator: Send + Sync {
    /// The Magical value as defined by EIP-1271
    const MAGICAL_VALUE: [u8; 4] = 0x1626ba7e_u32.to_le_bytes();

    async fn is_valid_signature(
        &self,
        contract_address: H160,
        hash: [u8; 32],
        signature: &[u8],
    ) -> Result<bool, IsValidSignatureError>;
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
impl SignatureValidator for Web3SignatureValidator {
    async fn is_valid_signature(
        &self,
        contract_address: H160,
        hash: [u8; 32],
        signature: &[u8],
    ) -> Result<bool, IsValidSignatureError> {
        let instance = contracts::SignatureValidator::at(&self.web3, contract_address);

        let is_valid_signature = instance
            .is_valid_signature(Bytes(hash), Bytes(signature.to_vec()))
            .call()
            .await
            .map_err(|_method_err| IsValidSignatureError::ContractCallFailed)?;

        Ok(is_valid_signature.0 == Self::MAGICAL_VALUE)
    }
}