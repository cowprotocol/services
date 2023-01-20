//! Liquidity fetching infrastructure.
//!
//! Note that this is part of the [`crate::infra`] module, and not
//! [`crate::domain`] module for a couple of reasons:
//!
//! 1. Liquidity fetching is inherently "infrastructure"-y in the same vein as
//!    [`crate::infra::blockchain::Ethereum`] is: they are both used for
//!    fetching blockchain state.
//! 2. There is a very concrete plan to move liquidity indexing into it's own
//!    service, at which point being it will truly fit in the [`crate::infra`]
//!    module.

use {crate::domain::eth, std::cmp::Ordering};

pub mod config;
pub mod fetcher;

pub use self::{config::Config, fetcher::Fetcher};

/// An ordered token pair.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TokenPair(eth::TokenAddress, eth::TokenAddress);

impl TokenPair {
    /// Returns a token pair for the given tokens, or `None` if `a` and `b` are
    /// equal.
    pub fn new(a: eth::TokenAddress, b: eth::TokenAddress) -> Option<Self> {
        match a.cmp(&b) {
            Ordering::Less => Some(Self(a, b)),
            Ordering::Equal => None,
            Ordering::Greater => Some(Self(b, a)),
        }
    }

    /// Returns the wrapped token pair as a tuple.
    pub fn get(&self) -> (eth::TokenAddress, eth::TokenAddress) {
        (self.0, self.1)
    }
}
