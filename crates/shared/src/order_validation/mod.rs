//! Modular order validation system
//!
//! This module provides a flexible, strategy-based order validation system that supports:
//!
//! - **Phase-based validation** - Intrinsic (sync) vs Extrinsic (async) checks
//! - **Strategy pattern** - Different validation logic for different order types
//! - **Modularity** - Easy to add new strategies or checks
//!
//! ## Overview
//!
//! The validation system has the following flow:
//!
//! 1. **Parse app data** (first-class operation) → `OrderAppData`
//! 2. **Select strategy** based on app_data.protocol.wrappers and signature
//! 3. **Run intrinsic validation** (sync) → `IntrinsicValidationResult`
//! 4. **Run extrinsic validation** (async) → `ExtrinsicValidationResult`
//! 5. **Construct validated order** → `ValidatedOrder`
//!
//! The validation system is split into two phases:
//! - **Intrinsic**: Synchronous checks of order properties (no blockchain state)
//! - **Extrinsic**: Asynchronous checks that require blockchain state
//!
//! Different order types (wrapper orders, EIP-1271 orders, regular orders) use different
//! validation strategies, which can be extended with new strategies for new order types.
//!
//! ## Usage
//!
//! ```ignore
//! use shared::order_validation::{OrderValidator, ValidationRequest, ValidationPhases};
//!
//! // Fast path: Intrinsic-only validation (for quotes)
//! let intrinsic = validator.validate_intrinsic(request)?;
//!
//! // Full validation: With phase control
//! let validated = validator.validate(request, ValidationPhases::ALL).await?;
//! ```

pub mod app_data;
pub mod checks;
pub mod phases;
pub mod strategies;
pub mod traits;
pub mod types;

// Re-export public API
pub use {
    app_data::parse_app_data,
    phases::ValidationPhases,
    traits::{OrderValidator, ValidationRequest, IntrinsicValidationResult, ValidatedOrder},
    types::{
        PreOrderData, OrderAppData, ValidationError, PartialValidationError,
        AppDataValidationError, OrderValidToError,
    },
};
