use std::time::SystemTime;

use anyhow::Result;

use crate::database::OrderFilter;
use crate::storage::{AddOrderResult, RemoveOrderResult, Storage};
use contracts::GPv2Settlement;
use model::{
    order::{Order, OrderCreation, OrderUid},
    DomainSeparator,
};

pub struct Orderbook {
    domain_separator: DomainSeparator,
    storage: Box<dyn Storage>,
}

impl Orderbook {
    pub fn new(domain_separator: DomainSeparator, storage: Box<dyn Storage>) -> Self {
        Self {
            domain_separator,
            storage,
        }
    }

    pub async fn add_order(&self, order: OrderCreation) -> Result<AddOrderResult> {
        if !has_future_valid_to(now_in_epoch_seconds(), &order) {
            return Ok(AddOrderResult::PastValidTo);
        }
        let order = match Order::from_order_creation(order, &self.domain_separator) {
            Some(order) => order,
            None => return Ok(AddOrderResult::InvalidSignature),
        };
        self.storage.add_order(order).await
    }

    pub async fn remove_order(&self, uid: &OrderUid) -> Result<RemoveOrderResult> {
        self.storage.remove_order(uid).await
    }

    pub async fn get_orders(&self, filter: &OrderFilter) -> Result<Vec<Order>> {
        self.storage.get_orders(filter).await
    }

    pub async fn run_maintenance(&self, settlement_contract: &GPv2Settlement) -> Result<()> {
        self.storage.run_maintenance(settlement_contract).await
    }
}

pub fn now_in_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("now earlier than epoch")
        .as_secs()
}

pub fn has_future_valid_to(now_in_epoch_seconds: u64, order: &OrderCreation) -> bool {
    order.valid_to as u64 > now_in_epoch_seconds
}
