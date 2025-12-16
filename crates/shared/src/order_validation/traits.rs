use {
    super::{phases::ValidationPhases, types::{ValidationError, OrderAppData, PreOrderData}},
    alloy::primitives::Address,
    async_trait::async_trait,
    model::{
        order::{Order, OrderClass, OrderCreation},
        quote::Quote,
        DomainSeparator,
        signature::SigningScheme,
    },
};

/// Unified order validation interface
///
/// Replaces the old `OrderValidating` trait with a more flexible API that supports
/// phase-based validation (intrinsic vs extrinsic) and multiple validation strategies
/// (regular orders, EIP-1271 orders, wrapper orders).
///
/// The validator supports two ways of validating:
/// 1. **Intrinsic-only** (synchronous): Fast path for quotes - validates only order properties
/// 2. **Full validation** (asynchronous): With phase control - validates with blockchain state
#[cfg_attr(any(test, feature = "test-util"), mockall::automock)]
#[async_trait::async_trait]
pub trait OrderValidator: Send + Sync {
    /// Synchronous validation of intrinsic properties only
    ///
    /// This is the fast path for quote validation - no blockchain state required.
    /// Returns an `IntrinsicValidationResult` that contains the owner, signing scheme,
    /// and parsed app data needed for extrinsic validation.
    ///
    /// This method never uses `async` or blockchain calls, making it suitable for
    /// the quote phase where performance is critical.
    fn validate_intrinsic(
        &self,
        request: ValidationRequest,
    ) -> Result<IntrinsicValidationResult, ValidationError>;

    /// Full asynchronous validation with phase control
    ///
    /// Validates an order with the specified validation phases:
    /// - `ValidationPhases::INTRINSIC` - Only order properties (sync)
    /// - `ValidationPhases::EXTRINSIC` - Only blockchain checks (async)
    /// - `ValidationPhases::ALL` - Both phases (async)
    ///
    /// Returns a `ValidatedOrder` containing the fully validated order, quote (if available),
    /// and metadata.
    ///
    /// Intrinsic validation is always required and cannot be skipped, as it's needed to
    /// determine which strategy to use and to parse the app data.
    async fn validate(
        &self,
        request: ValidationRequest,
        phases: ValidationPhases,
    ) -> Result<ValidatedOrder, ValidationError>;
}

/// Request structure for order validation
///
/// Contains all the information needed to validate an order, including the order itself,
/// domain separator for EIP-712 signature verification, and optional full app data override.
#[derive(Debug, Clone)]
pub struct ValidationRequest {
    /// The order creation request to validate
    pub order: OrderCreation,

    /// EIP-712 domain separator for signature verification
    pub domain_separator: DomainSeparator,

    /// Settlement contract address (used for signature verification)
    pub settlement_contract: Address,

    /// Optional full app data (used when order only specifies app data hash)
    pub full_app_data_override: Option<String>,
}

/// Result of successful intrinsic validation
///
/// Contains the minimal set of information extracted during intrinsic validation
/// (no blockchain state involved). This is returned by the fast-path `validate_intrinsic()`
/// method and also used internally during the full validation flow.
#[derive(Debug)]
pub struct IntrinsicValidationResult {
    /// Recovered owner address (from signature)
    pub owner: Address,

    /// Signing scheme used (ECDSA, EthSign, EIP-1271, or PreSign)
    pub signing_scheme: SigningScheme,

    /// Validated and parsed app data with rendered interactions
    pub app_data: OrderAppData,

    /// Pre-order data with intrinsic properties
    pub pre_order_data: PreOrderData,
}

/// Result of successful full validation
///
/// Contains the complete validated order along with metadata, including the
/// finally determined order class and any computed quote.
#[derive(Debug)]
pub struct ValidatedOrder {
    /// The fully constructed and validated order
    pub order: Order,

    /// Quote for the order (if available)
    ///
    /// Some orders don't have quotes (e.g., if only intrinsic validation was performed,
    /// or for liquidity orders).
    pub quote: Option<Quote>,

    /// Validated and parsed app data
    pub app_data: OrderAppData,

    /// Owner address (recovered from signature or specified in pre-sign)
    pub owner: Address,

    /// Final order class (may differ from input due to market price checks)
    pub class: OrderClass,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_request_can_be_cloned() {
        let request = ValidationRequest {
            order: Default::default(),
            domain_separator: Default::default(),
            settlement_contract: Default::default(),
            full_app_data_override: None,
        };

        let _cloned = request.clone();
    }
}
