//! Token quality and bad token detection
//!
//! This module handles validation of token quality.
//! All functions are **asynchronous** - they may require blockchain state queries
//! (e.g., checking token code, transfer simulation).

use {
    crate::order_validation::types::*,
    alloy::primitives::Address,
    bad_token::{BadTokenDetecting, TokenQuality},
    std::sync::Arc,
};

/// Checks if both buy and sell tokens pass bad token detection
///
/// # Async
/// This function is asynchronous and may query blockchain state to verify
/// token quality and detect potentially problematic tokens.
///
/// # Arguments
/// - `sell_token`: The token being sold
/// - `buy_token`: The token being bought
/// - `bad_token_detector`: The bad token detector trait implementation
///
/// # Returns
/// - `Ok(())` if both tokens are good
/// - `Err(ValidationError::Partial(...))` if either token is bad
pub async fn check_bad_tokens(
    sell_token: Address,
    buy_token: Address,
    bad_token_detector: &Arc<dyn BadTokenDetecting>,
) -> Result<(), ValidationError> {
    // Check both tokens
    for &token in &[sell_token, buy_token] {
        match bad_token_detector
            .detect(token)
            .await
            .map_err(|err| ValidationError::Partial(PartialValidationError::Other(err)))?
        {
            TokenQuality::Good => {
                // Token is good, continue
            }
            TokenQuality::Bad { reason } => {
                // Token failed bad token detection
                return Err(ValidationError::Partial(
                    PartialValidationError::UnsupportedToken { token, reason },
                ));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_check_stub() {
        // Tests will be implemented when full integration is ready
    }
}
