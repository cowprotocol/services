//! Base validation strategy trait
//!
//! This module defines the `ValidationStrategy` trait that all validation strategies must implement.
//! Different order types (regular, EIP-1271, wrapper) implement this trait with their own
//! validation logic.

use {
    super::super::types::*,
    alloy::primitives::Address,
    async_trait::async_trait,
    model::quote::Quote,
    std::sync::Arc,
};

/// Validation strategy for order validation
///
/// Different order types implement this trait to provide their own validation logic.
/// Each strategy must implement both intrinsic (sync) and extrinsic (async) validation.
#[async_trait::async_trait]
pub trait ValidationStrategy: Send + Sync {
    /// Human-readable name of this strategy (for logging/debugging)
    fn name(&self) -> &'static str;

    /// Synchronous intrinsic validation (no blockchain state)
    ///
    /// This method must not use `async` and should only validate order properties
    /// that don't depend on blockchain state. It's called by the fast-path
    /// `validate_intrinsic()` method for quote validation.
    fn validate_intrinsic(
        &self,
        context: &ValidationContext,
    ) -> Result<IntrinsicValidationResult, ValidationError>;

    /// Asynchronous extrinsic validation (requires blockchain state)
    ///
    /// This method validates checks that require blockchain state or external calls,
    /// such as balance checks, signature verification, and quote calculation.
    async fn validate_extrinsic(
        &self,
        context: &ValidationContext,
        intrinsic_result: &IntrinsicValidationResult,
    ) -> Result<ExtrinsicValidationResult, ValidationError>;
}

/// Context passed to strategies containing all dependencies
///
/// This struct holds all the dependencies (traits, configs, instances) needed
/// by validation strategies to perform their checks. Rather than each strategy
/// constructor taking 13+ parameters, we pass a single context with all needed
/// information.
pub struct ValidationContext<'a> {
    // Core request data
    pub request: &'a super::super::traits::ValidationRequest,

    // Native token (WETH) instance
    pub native_token: &'a contracts::alloy::WETH9::Instance,

    // Banned user detection
    pub banned_users: &'a Arc<order_validation::banned::Users>,

    // Configuration
    pub validity_configuration: &'a OrderValidPeriodConfiguration,

    // Bad token detection
    pub bad_token_detector: &'a Arc<dyn bad_token::BadTokenDetecting>,

    // Hooks for pre/post-settlement interactions
    pub hooks: &'a contracts::alloy::HooksTrampoline::Instance,

    // Quote calculation
    pub quoter: &'a Arc<dyn order_quoting::OrderQuoting>,

    // Balance and allowance checks
    pub balance_fetcher: &'a Arc<dyn account_balances::BalanceFetching>,

    // Signature validation
    pub signature_validator: &'a Arc<dyn signature_validator::SignatureValidating>,

    // Limit order counting
    pub limit_order_counter: &'a Arc<dyn order_validation::limit_orders::LimitOrderCounting>,

    // App data validation
    pub app_data_validator: &'a app_data::Validator,

    // Configuration limits
    pub max_limit_orders_per_user: u64,
    pub max_gas_per_order: u64,

    // Feature flags
    pub eip1271_skip_creation_validation: bool,
}

/// Result of intrinsic validation
///
/// Contains the information extracted during intrinsic validation that's needed
/// for extrinsic validation and order construction.
#[derive(Debug)]
pub struct IntrinsicValidationResult {
    pub owner: Address,
    pub signing_scheme: model::signature::SigningScheme,
    pub app_data: OrderAppData,
    pub pre_order_data: PreOrderData,
}

/// Result of extrinsic validation
///
/// Contains the information computed during extrinsic validation, particularly
/// the quote and final order class.
#[derive(Debug)]
pub struct ExtrinsicValidationResult {
    pub quote: Option<Quote>,
    pub class: model::order::OrderClass,
    pub verification_gas_limit: u64,
}

#[cfg(test)]
mod tests {
    // Tests will be added when we implement the concrete strategies
}
