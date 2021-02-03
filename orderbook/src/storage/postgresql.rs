use super::*;
use crate::{
    database::{Database, OrderFilter},
    event_updater::EventUpdater,
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use futures::TryStreamExt;
use model::order::{Order, OrderUid};

pub struct OrderBook {
    database: Database,
    event_updater: EventUpdater,
}

impl OrderBook {
    pub fn new(contract: GPv2Settlement, database: Database) -> Self {
        Self {
            database: database.clone(),
            event_updater: EventUpdater::new(contract, database),
        }
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
        self.event_updater
            .update_events()
            .await
            .context("event updater failed")
    }
}
