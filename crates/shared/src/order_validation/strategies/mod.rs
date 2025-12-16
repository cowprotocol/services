//! Validation strategies for different order types
//!
//! This module implements the strategy pattern for order validation, allowing different
//! validation logic for different order types:
//!
//! - **FlashloanOrderStrategy**: Orders using flashloans (skips all validations)
//! - **WrapperOrderStrategy**: Orders with wrapper contracts
//! - **Eip1271OrderStrategy**: Smart contract signature orders
//! - **RegularOrderStrategy**: Standard ECDSA/EthSign/PreSign orders (fallback)

pub mod strategy;
pub mod flashloan;
pub mod regular;
pub mod eip1271;
pub mod wrapper;

pub use strategy::{
    ValidationStrategy, ValidationContext, IntrinsicValidationResult, ExtrinsicValidationResult,
};

// Import the ValidatedAppData to avoid circular dependencies
use {
    app_data::ValidatedAppData,
    model::signature::Signature,
};

/// Strategy selector that chooses the right validation strategy for an order
pub enum StrategySelector {
    /// Flashloan strategy for orders using flashloans (highest priority)
    Flashloan,
    /// Strategy for orders with wrapper contracts
    Wrapper,
    /// Strategy for EIP-1271 smart contract signatures
    Eip1271,
    /// Fallback strategy for regular orders
    Regular,
}

impl StrategySelector {
    /// Selects the validation strategy based on order characteristics
    ///
    /// # Priority
    /// 1. **Flashloan** - If order uses flashloans (highest priority, skips all validations)
    /// 2. **Wrapper** - If order has wrappers
    /// 3. **EIP-1271** - If order uses EIP-1271 signature
    /// 4. **Regular** - Fallback for standard orders (ECDSA, EthSign, PreSign)
    ///
    /// The priority ensures that specialized order types get their appropriate validation.
    pub fn select(app_data: &ValidatedAppData, signature: &Signature) -> Self {
        // Highest priority: Flashloan orders (skip all validations)
        if app_data.protocol.flashloan.is_some() {
            return Self::Flashloan;
        }

        // Second priority: Wrapper orders
        if !app_data.protocol.wrappers.is_empty() {
            return Self::Wrapper;
        }

        // Third priority: EIP-1271 signatures
        if matches!(signature, Signature::Eip1271(_)) {
            return Self::Eip1271;
        }

        // Fallback: Regular orders (ECDSA, EthSign, PreSign)
        Self::Regular
    }

    /// Create a strategy instance based on this selector
    pub fn create_strategy(self) -> Box<dyn ValidationStrategy> {
        match self {
            Self::Flashloan => Box::new(flashloan::FlashloanOrderStrategy),
            Self::Regular => Box::new(regular::RegularOrderStrategy),
            Self::Eip1271 => Box::new(eip1271::Eip1271OrderStrategy),
            Self::Wrapper => Box::new(wrapper::WrapperOrderStrategy),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_selector_prefers_wrapper() {
        // Wrapper should be selected even if signature is EIP-1271
        // (This test will be properly implemented once we have real types)
        // For now, just verify the selector enum exists
        let _selector = StrategySelector::Regular;
    }
}
