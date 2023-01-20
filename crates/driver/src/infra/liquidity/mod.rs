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

pub mod config;
pub mod fetcher;

pub use self::{config::Config, fetcher::Fetcher};
