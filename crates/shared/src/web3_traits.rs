//! Module containing traits for abstracting Web3 operations so components can
//! more easily be tested with mocked versions of these behaviours.

use crate::Web3;
use anyhow::Result;
use web3::types::{Bytes, H160};

#[mockall::automock]
#[async_trait::async_trait]
pub trait CodeFetching: Send + Sync {
    /// Fetches the code size at the specified address.
    async fn code(&self, address: H160) -> Result<Bytes>;

    /// Fetches the code for the specified address.
    async fn code_size(&self, address: H160) -> Result<usize>;
}

#[async_trait::async_trait]
impl CodeFetching for Web3 {
    async fn code(&self, address: H160) -> Result<Bytes> {
        Ok(self.eth().code(address, None).await?)
    }

    async fn code_size(&self, address: H160) -> Result<usize> {
        Ok(self.code(address).await?.0.len())
    }
}
