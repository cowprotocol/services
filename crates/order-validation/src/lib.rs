//! This crate is intended to contain code that is required to perform order
//! validation.

pub mod wrappers;

/// Banned user detection for EVM addresses.
pub mod banned {
    pub use banned_users::HermodConfig;

    /// The banned EVM addresses.
    pub type Users = banned_users::Users<alloy_primitives::Address>;
}
