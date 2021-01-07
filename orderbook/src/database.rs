use anyhow::Result;
use model::order::OrderUid;
use primitive_types::U256;

pub struct Trade {
    pub block_number: u64,
    pub log_index: u64,
    pub order_uid: OrderUid,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
}

/// Abstraction over the database we use to store persistent data like orders and trade events.
#[async_trait::async_trait]
pub trait Database {
    async fn block_number_of_most_recent_trade(&self) -> Result<u64>;
    // All insertions happen in one transaction.
    async fn insert_trades(&self, trades: &[Trade]) -> Result<()>;
    // The deletion and all insertions happen in one transaction.
    async fn replace_trades(
        &self,
        delete_from_block_number: u64,
        new_trades: &[Trade],
    ) -> Result<()>;
}
