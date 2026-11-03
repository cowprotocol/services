//! Serialization utilities for use with [`serde_with::serde_as`] macros.

mod hex;
mod nonempty;
#[cfg(feature = "solana")]
mod pubkey;
mod u256;
mod url;

#[cfg(feature = "solana")]
pub use self::pubkey::{deserialize_optional_solana_pubkey_b58, deserialize_solana_pubkey_b58};
pub use self::{
    hex::Hex,
    nonempty::{deserialize_nonempty_unique_vec, deserialize_nonempty_vec},
    u256::U256,
    url::deserialize_url_with_trailing_slash,
};
