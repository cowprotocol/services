use super::{orders::OrderStoring, trades::TradeRetrieving, Postgres};
use crate::order_quoting::QuoteStoring;
use ethcontract::H256;
use model::order::Order;
use prometheus::Histogram;
use shared::{event_handling::EventStoring, maintenance::Maintaining};
use std::sync::Arc;

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Instrumented {
    inner: Postgres,
    metrics: Arc<dyn Metrics>,
}

impl Instrumented {
    pub fn new(inner: Postgres, metrics: Arc<dyn Metrics>) -> Self {
        Self { inner, metrics }
    }
}

pub trait Metrics: Send + Sync {
    fn database_query_histogram(&self, label: &str) -> Histogram;
}

#[async_trait::async_trait]
impl EventStoring<contracts::gpv2_settlement::Event> for Instrumented {
    async fn replace_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
        range: std::ops::RangeInclusive<shared::event_handling::BlockNumber>,
    ) -> anyhow::Result<()> {
        let _timer = self
            .metrics
            .database_query_histogram("replace_events")
            .start_timer();
        self.inner.replace_events(events, range).await
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> anyhow::Result<()> {
        let _timer = self
            .metrics
            .database_query_histogram("append_events")
            .start_timer();
        self.inner.append_events(events).await
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let _timer = self
            .metrics
            .database_query_histogram("last_event_block")
            .start_timer();
        self.inner.last_event_block().await
    }
}

#[async_trait::async_trait]
impl QuoteStoring for Instrumented {
    async fn save(
        &self,
        data: crate::order_quoting::QuoteData,
    ) -> anyhow::Result<Option<model::quote::QuoteId>> {
        let _timer = self
            .metrics
            .database_query_histogram("save_quote")
            .start_timer();
        self.inner.save(data).await
    }

    async fn get(
        &self,
        id: model::quote::QuoteId,
    ) -> anyhow::Result<Option<crate::order_quoting::QuoteData>> {
        let _timer = self
            .metrics
            .database_query_histogram("get_quote")
            .start_timer();
        self.inner.get(id).await
    }

    async fn find(
        &self,
        parameters: crate::order_quoting::QuoteSearchParameters,
        expiration: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<Option<(model::quote::QuoteId, crate::order_quoting::QuoteData)>> {
        let _timer = self
            .metrics
            .database_query_histogram("find_quote")
            .start_timer();
        self.inner.find(parameters, expiration).await
    }
}

#[async_trait::async_trait]
impl OrderStoring for Instrumented {
    async fn insert_order(
        &self,
        order: &model::order::Order,
        quote: Option<crate::order_quoting::Quote>,
    ) -> anyhow::Result<(), super::orders::InsertionError> {
        let _timer = self
            .metrics
            .database_query_histogram("insert_order")
            .start_timer();
        self.inner.insert_order(order, quote).await
    }

    async fn cancel_order(
        &self,
        order_uid: &model::order::OrderUid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<()> {
        let _timer = self
            .metrics
            .database_query_histogram("cancel_order")
            .start_timer();
        self.inner.cancel_order(order_uid, now).await
    }

    async fn replace_order(
        &self,
        old_order: &model::order::OrderUid,
        new_order: &model::order::Order,
        new_quote: Option<crate::order_quoting::Quote>,
    ) -> anyhow::Result<(), super::orders::InsertionError> {
        let _timer = self
            .metrics
            .database_query_histogram("replace_order")
            .start_timer();
        self.inner
            .replace_order(old_order, new_order, new_quote)
            .await
    }

    async fn orders(
        &self,
        filter: &super::orders::OrderFilter,
    ) -> anyhow::Result<Vec<model::order::Order>> {
        let _timer = self
            .metrics
            .database_query_histogram("orders")
            .start_timer();
        self.inner.orders(filter).await
    }

    async fn orders_for_tx(&self, tx_hash: &H256) -> anyhow::Result<Vec<Order>> {
        let _timer = self
            .metrics
            .database_query_histogram("orders_for_tx")
            .start_timer();
        self.inner.orders_for_tx(tx_hash).await
    }

    async fn single_order(
        &self,
        uid: &model::order::OrderUid,
    ) -> anyhow::Result<Option<model::order::Order>> {
        let _timer = self
            .metrics
            .database_query_histogram("single_order")
            .start_timer();
        self.inner.single_order(uid).await
    }

    async fn solvable_orders(
        &self,
        min_valid_to: u32,
    ) -> anyhow::Result<super::orders::SolvableOrders> {
        let _timer = self
            .metrics
            .database_query_histogram("solvable_orders")
            .start_timer();
        self.inner.solvable_orders(min_valid_to).await
    }

    async fn user_orders(
        &self,
        owner: &ethcontract::H160,
        offset: u64,
        limit: Option<u64>,
    ) -> anyhow::Result<Vec<model::order::Order>> {
        let _timer = self
            .metrics
            .database_query_histogram("user_orders")
            .start_timer();
        self.inner.user_orders(owner, offset, limit).await
    }
}

#[async_trait::async_trait]
impl TradeRetrieving for Instrumented {
    async fn trades(
        &self,
        filter: &super::trades::TradeFilter,
    ) -> anyhow::Result<Vec<model::trade::Trade>> {
        let _timer = self
            .metrics
            .database_query_histogram("trades")
            .start_timer();
        self.inner.trades(filter).await
    }
}

#[async_trait::async_trait]
impl Maintaining for Instrumented {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        let _timer = self
            .metrics
            .database_query_histogram("remove_expired_quotes")
            .start_timer();
        self.inner.run_maintenance().await
    }
}
