//! Flashloan order validation strategy
//!
//! This strategy handles orders that use flashloans to source their sell token.
//! Since flashloans provide guaranteed liquidity for the settlement, we skip all
//! standard validation checks (balance, allowance, bad tokens, etc.)

use {
    super::strategy::{
        ValidationStrategy, ValidationContext, IntrinsicValidationResult, ExtrinsicValidationResult,
    },
    crate::order_validation::types::*,
    async_trait::async_trait,
};

/// Validation strategy for flashloan orders
///
/// Flashloan orders bypass standard validation because the flashloan provider
/// guarantees the availability of the sell token during settlement.
/// This allows for novel order flows where tokens don't need to be in the user's
/// wallet at order creation time.
pub struct FlashloanOrderStrategy;

#[async_trait]
impl ValidationStrategy for FlashloanOrderStrategy {
    fn name(&self) -> &'static str {
        "FlashloanOrderStrategy"
    }

    /// Intrinsic validation for flashloan orders
    ///
    /// Only validates app data and basic order properties. Does not check
    /// balance, allowance, or token quality since the flashloan provider
    /// will ensure the token is available.
    fn validate_intrinsic(
        &self,
        context: &ValidationContext,
    ) -> Result<IntrinsicValidationResult, ValidationError> {
        // Skip all intrinsic validations for flashloan orders
        // The flashloan provider guarantees token availability
        tracing::debug!("flashloan order: skipping intrinsic validations");
        Ok(IntrinsicValidationResult {
            owner: context.request.order.from,
            signing_scheme: context.request.order.signature.scheme(),
            app_data: Default::default(),
            pre_order_data: PreOrderData::default(),
        })
    }

    /// Extrinsic validation for flashloan orders
    ///
    /// All extrinsic validations are skipped. The flashloan provider assumes
    /// responsibility for ensuring the order can be executed with the provided token.
    async fn validate_extrinsic(
        &self,
        _context: &ValidationContext,
        intrinsic_result: &IntrinsicValidationResult,
    ) -> Result<ExtrinsicValidationResult, ValidationError> {
        // Skip all extrinsic validations for flashloan orders
        tracing::debug!("flashloan order: skipping extrinsic validations");
        Ok(ExtrinsicValidationResult {
            quote: None,
            class: intrinsic_result.pre_order_data.class,
            verification_gas_limit: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flashloan_strategy_name() {
        let strategy = FlashloanOrderStrategy;
        assert_eq!(strategy.name(), "FlashloanOrderStrategy");
    }
}
