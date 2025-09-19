use {
    crate::price_estimation::trade_verifier::balance_overrides::{
        BalanceOverrideRequest,
        BalanceOverriding,
    },
    ethcontract::Bytes,
    ethrpc::Web3,
    hex_literal::hex,
    model::interaction::InteractionData,
    primitive_types::H160,
    std::sync::Arc,
    thiserror::Error,
};

mod simulation;

/// Structure used to represent a signature.
#[derive(Clone, Eq, PartialEq)]
pub struct SignatureCheck {
    pub signer: H160,
    pub hash: [u8; 32],
    pub signature: Vec<u8>,
    pub interactions: Vec<InteractionData>,
    pub balance_override: Option<BalanceOverrideRequest>,
}

impl SignatureCheck {
    fn requires_setup(&self) -> bool {
        !self.interactions.is_empty() || self.balance_override.is_some()
    }
}

impl std::fmt::Debug for SignatureCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureCheck")
            .field("signer", &self.signer)
            .field("hash", &format_args!("0x{}", hex::encode(self.hash)))
            .field(
                "signature",
                &format_args!("0x{}", hex::encode(&self.signature)),
            )
            .field("interactions", &self.interactions)
            .finish()
    }
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

/// Contracts required for signature verification simulation.
pub struct Contracts {
    pub settlement: contracts::GPv2Settlement,
    pub signatures: contracts::support::Signatures,
    pub vault_relayer: H160,
}

/// Creates the default [`SignatureValidating`] instance.
pub fn validator(
    web3: &Web3,
    contracts: Contracts,
    balance_overrider: Arc<dyn BalanceOverriding>,
) -> Arc<dyn SignatureValidating> {
    Arc::new(simulation::Validator::new(
        web3,
        contracts.settlement,
        contracts.signatures,
        contracts.vault_relayer,
        balance_overrider,
    ))
}
