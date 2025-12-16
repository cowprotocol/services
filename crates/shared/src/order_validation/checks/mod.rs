//! Individual validation check functions
//!
//! This module contains granular validation check functions, each in its own file.
//! Functions are inherently sync or async based on their signature, not based on
//! which file they're in. Strategies compose these checks as needed for their
//! validation phase logic.
//!
//! **Note**: App data parsing is NOT in this directory - it's a top-level first-class
//! operation in `app_data.rs` that runs before strategy selection.
//!
//! ## Check Modules
//!
//! - `intrinsic.rs` - Basic order property validation (sync)
//! - `signature.rs` - Signature recovery and EIP-1271 validation (mixed sync/async)
//! - `balance.rs` - Balance, allowance, and transfer simulation checks (async)
//! - `token.rs` - Token quality and bad token detection (async)
//! - `quote.rs` - Quote calculation and price validation (async)

pub mod intrinsic;
pub mod balance;
pub mod quote;
pub mod signature;
pub mod token;

// Re-export for convenient access
pub use {
    intrinsic::*,
    balance::*,
    quote::*,
    signature::*,
    token::*,
};
