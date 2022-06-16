use anyhow::{Context, Result};
use ethcontract::Bytes;
use hex_literal::hex;
use primitive_types::H160;
use shared::Web3;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
/// <https://eips.ethereum.org/EIPS/eip-1271>
pub trait SignatureValidator: Send + Sync {
    async fn is_valid_signature(
        &self,
        contract_address: H160,
        hash: [u8; 32],
        signature: &[u8],
    ) -> Result<bool>;
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
impl SignatureValidator for Web3SignatureValidator {
    async fn is_valid_signature(
        &self,
        contract_address: H160,
        hash: [u8; 32],
        signature: &[u8],
    ) -> Result<bool> {
        let instance = contracts::ERC1271SignatureValidator::at(&self.web3, contract_address);

        let is_valid_signature = instance
            .is_valid_signature(Bytes(hash), Bytes(signature.to_vec()))
            .call()
            .await
            .context("isValidSignature")?;

        Ok(is_valid_signature.0 == Self::MAGICAL_VALUE)
    }
}
