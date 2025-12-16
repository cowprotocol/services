//! Signature validation and owner recovery
//!
//! This module handles signature recovery and validation.
//! Contains both **synchronous** functions (ECDSA/EthSign recovery) and
//! **asynchronous** functions (EIP-1271 on-chain verification).

use {
    crate::order_validation::types::*,
    alloy::primitives::Address,
    ethcontract::H256,
    model::{
        DomainSeparator,
        order::{OrderCreation, OrderData},
        signature::{Signature, SigningScheme, hashed_eip712_message},
        quote::QuoteSigningScheme,
    },
    signature_validator::{SignatureValidating, SignatureCheck, SignatureValidationError},
    price_estimation::trade_verifier::balance_overrides::BalanceOverrideRequest,
    std::sync::Arc,
};

/// Recovers the order owner from an ECDSA or EthSign signature
///
/// # Sync
/// This function is synchronous - it only performs cryptographic operations
/// without any blockchain calls.
///
/// # Arguments
/// - `order`: The order creation request containing the signature
/// - `domain_separator`: EIP-712 domain separator for signature verification
/// - `app_data_signer`: Optional alternative signer from app data
///
/// # Returns
/// The recovered owner address, or a validation error if recovery fails
pub fn recover_owner(
    order: &OrderCreation,
    domain_separator: &DomainSeparator,
    app_data_signer: Option<Address>,
) -> Result<Address, ValidationError> {
    order
        .verify_owner(domain_separator, app_data_signer)
        .map_err(ValidationError::from)
}

/// Validates an EIP-1271 smart contract signature on-chain
///
/// # Async
/// This function is asynchronous and requires an on-chain call to verify
/// the signature via `isValidSignature()`.
///
/// Returns the additional gas cost for signature verification if valid.
///
/// # Arguments
/// - `signature`: The EIP-1271 signature bytes
/// - `owner`: The smart contract address that signed
/// - `domain_separator`: EIP-712 domain separator
/// - `data`: The order data being signed
/// - `signature_validator`: The signature validator trait implementation
/// - `pre_interactions`: Pre-settlement interactions (for balance override context)
/// - `flashloan`: Optional flashloan data (for balance override)
/// - `skip_validation`: If true, skip on-chain validation and return 0 gas
///
/// # Returns
/// The gas limit needed for signature verification, or a validation error
pub async fn validate_eip1271_if_needed(
    signature: &[u8],
    owner: Address,
    domain_separator: &DomainSeparator,
    data: &OrderData,
    signature_validator: &Arc<dyn SignatureValidating>,
    pre_interactions: &[model::interaction::InteractionData],
    flashloan: Option<&app_data::Flashloan>,
    skip_validation: bool,
) -> Result<u64, ValidationError> {
    if skip_validation {
        tracing::debug!("skipping EIP-1271 signature validation");
        return Ok(0);
    }

    let hash = hashed_eip712_message(domain_separator, &data.hash_struct());

    signature_validator
        .validate_signature_and_get_additional_gas(SignatureCheck {
            signer: owner.into(),
            hash,
            signature: signature.to_vec(),
            interactions: pre_interactions.to_vec(),
            balance_override: flashloan.map(|loan| BalanceOverrideRequest {
                token: loan.token.into(),
                holder: loan.receiver.into(),
                amount: loan.amount.into(),
            }),
        })
        .await
        .map_err(|err| match err {
            SignatureValidationError::Invalid => {
                ValidationError::InvalidEip1271Signature(H256(hash))
            }
            SignatureValidationError::Other(err) => ValidationError::Other(err),
        })
}

/// Converts a signing scheme to a quote signing scheme
///
/// # Sync
/// This function is synchronous - it's a pure conversion function.
///
/// # Arguments
/// - `scheme`: The order signing scheme
/// - `order_placement_via_api`: Whether the order is being placed via API
/// - `verification_gas_limit`: Gas cost for EIP-1271 signature verification
///
/// # Returns
/// A `QuoteSigningScheme` suitable for quote calculation
pub fn convert_signing_scheme_to_quote_scheme(
    scheme: SigningScheme,
    order_placement_via_api: bool,
    verification_gas_limit: u64,
) -> Result<QuoteSigningScheme, ()> {
    match (order_placement_via_api, scheme) {
        (true, SigningScheme::Eip712) => Ok(QuoteSigningScheme::Eip712),
        (true, SigningScheme::EthSign) => Ok(QuoteSigningScheme::EthSign),
        (false, SigningScheme::Eip712) => Err(()),
        (false, SigningScheme::EthSign) => Err(()),
        (order_placement_via_api, SigningScheme::PreSign) => Ok(QuoteSigningScheme::PreSign {
            onchain_order: !order_placement_via_api,
        }),
        (order_placement_via_api, SigningScheme::Eip1271) => Ok(QuoteSigningScheme::Eip1271 {
            onchain_order: !order_placement_via_api,
            verification_gas_limit,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_signing_scheme_eip712_api_placement() {
        let scheme = convert_signing_scheme_to_quote_scheme(SigningScheme::Eip712, true, 0);
        assert!(matches!(scheme, Ok(QuoteSigningScheme::Eip712)));
    }

    #[test]
    fn convert_signing_scheme_presign() {
        let scheme = convert_signing_scheme_to_quote_scheme(SigningScheme::PreSign, true, 0);
        assert!(matches!(scheme, Ok(QuoteSigningScheme::PreSign { onchain_order: false })));
    }
}
