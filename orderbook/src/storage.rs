mod memory;
mod postgresql;

use crate::database::OrderFilter;
use anyhow::Result;
use contracts::GPv2Settlement;
use model::order::{Order, OrderUid};

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
    async fn add_order(&self, order: Order) -> Result<AddOrderResult>;
    async fn remove_order(&self, uid: &OrderUid) -> Result<RemoveOrderResult>;
    async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>>;
    async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) -> Result<()>;
}
