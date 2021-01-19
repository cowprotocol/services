mod memory;
mod postgresql;

use crate::database::OrderFilter;
use anyhow::Result;
use contracts::GPv2Settlement;
use model::order::{Order, OrderCreation, OrderUid};
use std::time::SystemTime;

pub use memory::OrderBook as InMemoryOrderBook;

#[derive(Debug, Eq, PartialEq)]
pub enum AddOrderResult {
    Added(OrderUid),
    DuplicatedOrder,
    InvalidSignature,
    Forbidden,
    MissingOrderData,
    PastValidTo,
    InsufficientFunds,
}

#[derive(Debug)]
pub enum RemoveOrderResult {
    Removed,
    DoesNotExist,
}

#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn add_order(&self, order: OrderCreation) -> Result<AddOrderResult>;
    async fn remove_order(&self, uid: &OrderUid) -> Result<RemoveOrderResult>;
    async fn get_orders(&self, filter: &OrderFilter<'_>) -> Result<Vec<Order>>;
    async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) -> Result<()>;
}

fn now_in_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("now earlier than epoch")
        .as_secs()
}

fn has_future_valid_to(now_in_epoch_seconds: u64, order: &OrderCreation) -> bool {
    order.valid_to as u64 > now_in_epoch_seconds
}
