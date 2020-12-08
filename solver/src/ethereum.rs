use crate::settlement::Settlement;
use anyhow::Result;
use async_trait::async_trait;
use model::TokenPair;
use primitive_types::H160;

// Not final at all. Just want some prototypes available to use in other code.

#[async_trait]
pub trait SettlementContract {
    async fn get_nonce(&self, token_pair: TokenPair) -> Result<u32>;
    // Call simulates the transaction.
    async fn settle_call(&self, settlement: &Settlement) -> Result<()>;
    // Send really executes the transaction.
    async fn settle_send(&self, settlement: &Settlement) -> Result<()>;
}

#[async_trait]
pub trait ERC20 {
    async fn balance_of(&self, owner: H160) -> Result<u128>;
    async fn allowance(&self, owner: H160, spender: H160) -> Result<bool>;
}
