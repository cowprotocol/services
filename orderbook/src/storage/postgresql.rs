use super::*;
use crate::database::{Database, OrderFilter};
use anyhow::Result;
use contracts::GPv2Settlement;
use futures::TryStreamExt;
use model::order::{Order, OrderUid};

pub struct OrderBook {
    database: Database,
}

impl OrderBook {
    pub fn _new(database: Database) -> Self {
        Self { database }
    }
}

#[async_trait::async_trait]
impl Storage for OrderBook {
    async fn add_order(&self, order: Order) -> Result<AddOrderResult> {
        self.database.insert_order(&order).await?;
        Ok(AddOrderResult::Added(order.order_meta_data.uid))
    }

    async fn remove_order(&self, _uid: &OrderUid) -> Result<RemoveOrderResult> {
        todo!()
    }

    async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        self.database.orders(filter).try_collect::<Vec<_>>().await
    }

    async fn run_maintenance(&self, _settlement_contract: &GPv2Settlement) -> Result<()> {
        Ok(())
    }
}
