mod memory;
mod postgresql;

use crate::database::{Database, OrderFilter};
use anyhow::Result;
use contracts::GPv2Settlement;
use model::order::{Order, OrderUid};
use url::Url;

pub use memory::OrderBook as InMemoryOrderBook;

pub async fn postgres_orderbook(contract: GPv2Settlement, db_url: Url) -> Result<impl Storage> {
    let db = Database::new(db_url.as_str())?;
    let order_book = postgresql::OrderBook::new(contract, db);
    // Perform one operation on the database to ensure that the connection works.
    order_book
        .get_orders(&OrderFilter {
            uid: Some(OrderUid::default()),
            ..Default::default()
        })
        .await?;
    Ok(order_book)
}

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

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait Storage: Send + Sync {
    async fn add_order(&self, order: Order) -> Result<AddOrderResult>;
    async fn remove_order(&self, uid: &OrderUid) -> Result<RemoveOrderResult>;
    async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>>;
    async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) -> Result<()>;
}
