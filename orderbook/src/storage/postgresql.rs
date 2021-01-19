use super::*;
use crate::database::{Database, OrderFilter};
use anyhow::Result;
use contracts::GPv2Settlement;
use futures::TryStreamExt;
use model::{
    order::{Order, OrderCreation, OrderUid},
    DomainSeparator,
};

pub struct OrderBook {
    domain_separator: DomainSeparator,
    database: Database,
}

impl OrderBook {
    pub fn _new(domain_separator: DomainSeparator, database: Database) -> Self {
        Self {
            domain_separator,
            database,
        }
    }
}

#[async_trait::async_trait]
impl Storage for OrderBook {
    async fn add_order(&self, order: OrderCreation) -> Result<AddOrderResult> {
        if !has_future_valid_to(now_in_epoch_seconds(), &order) {
            return Ok(AddOrderResult::PastValidTo);
        }
        let order = match Order::from_order_creation(order, &self.domain_separator) {
            Some(order) => order,
            None => return Ok(AddOrderResult::InvalidSignature),
        };
        self.database.insert_order(&order).await?;
        Ok(AddOrderResult::Added(order.order_meta_data.uid))
    }

    async fn remove_order(&self, _uid: &OrderUid) -> Result<RemoveOrderResult> {
        todo!()
    }

    async fn get_orders(&self, filter: &OrderFilter<'_>) -> Result<Vec<Order>> {
        self.database.orders(filter).try_collect::<Vec<_>>().await
    }

    async fn run_maintenance(&self, _settlement_contract: &GPv2Settlement) -> Result<()> {
        Ok(())
    }
}
