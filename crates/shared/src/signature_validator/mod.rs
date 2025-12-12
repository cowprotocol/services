use {
    crate::price_estimation::trade_verifier::balance_overrides::{
        BalanceOverrideRequest,
        BalanceOverriding,
    },
    alloy::primitives::{Address, FixedBytes},
    contracts::alloy::GPv2Settlement,
    ethrpc::Web3,
    hex_literal::hex,
    model::interaction::InteractionData,
    std::sync::Arc,
    thiserror::Error,
};

mod simulation;

/// Structure used to represent a signature.
#[derive(Clone, Eq, PartialEq)]
pub struct SignatureCheck {
    pub signer: Address,
    pub hash: [u8; 32],
    pub signature: Vec<u8>,
    pub interactions: Vec<InteractionData>,
    pub balance_override: Option<BalanceOverrideRequest>,
}

impl SignatureCheck {
    /// A signature check requires setup when there are interactions to be taken
    /// into account or when the balance override is set.
    ///
    /// Interactions require setup because a trader may not be able to trade
    /// without the pre-interaction, consider a case where, through the
    /// pre-interaction, the trader claims an airdrop, giving them enough
    /// balance to perform the trade; thus we need to simulate the
    /// pre-interaction first to validate whether the trader can actually
    /// perform the trade.
    ///
    /// The balance override is a simple way to test for things like flashloans,
    /// where we simply assume the user will have X funds.
    fn requires_setup(&self) -> bool {
        !self.interactions.is_empty() || self.balance_override.is_some()
    }
}

impl std::fmt::Debug for SignatureCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignatureCheck")
            .field("signer", &self.signer)
            .field("hash", &const_hex::encode_prefixed(self.hash))
            .field("signature", &const_hex::encode_prefixed(&self.signature))
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

#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
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

pub fn check_erc1271_result(result: FixedBytes<4>) -> Result<(), SignatureValidationError> {
    if result.0 == MAGICAL_VALUE {
        Ok(())
    } else {
        Err(SignatureValidationError::Invalid)
    }
}

/// Contracts required for signature verification simulation.
pub struct Contracts {
    pub settlement: GPv2Settlement::Instance,
    pub signatures: contracts::alloy::support::Signatures::Instance,
    pub vault_relayer: Address,
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
        *contracts.signatures.address(),
        contracts.vault_relayer,
        balance_overrider,
    ))
}
