use crate::settlement::Settlement;
use anyhow::Result;
use async_trait::async_trait;
use model::TokenPair;
use primitive_types::H160;

// Not final at all. Just want some prototypes available to use in other code.

#[async_trait]
pub trait SettlementContract {
    async fn get_nonce(&self, token_pair: TokenPair) -> Result<u32>;
    async fn settle(&self, settlement: Settlement) -> Result<()>;
}

#[async_trait]
pub trait Ethereum {
    async fn get_erc20_balance(&self, user: H160, token: H160) -> Result<u128>;
    async fn is_spending_approved(&self, user: H160, token: H160, spender: H160) -> Result<bool>;
    async fn get_uniswap_price(&self, token_pair: TokenPair) -> Result<f64>;
}
