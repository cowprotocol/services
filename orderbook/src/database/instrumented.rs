use super::{orders::OrderStoring, trades::TradeRetrieving, Postgres};
use crate::fee::MinFeeStoring;
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
impl MinFeeStoring for Instrumented {
    async fn save_fee_measurement(
        &self,
        sell_token: ethcontract::H160,
        buy_token: Option<ethcontract::H160>,
        amount: Option<ethcontract::U256>,
        kind: Option<model::order::OrderKind>,
        expiry: chrono::DateTime<chrono::Utc>,
        min_fee: ethcontract::U256,
    ) -> anyhow::Result<()> {
        let _timer = self
            .metrics
            .database_query_histogram("save_fee_measurement")
            .start_timer();
        self.inner
            .save_fee_measurement(sell_token, buy_token, amount, kind, expiry, min_fee)
            .await
    }

    async fn get_min_fee(
        &self,
        sell_token: ethcontract::H160,
        buy_token: Option<ethcontract::H160>,
        amount: Option<ethcontract::U256>,
        kind: Option<model::order::OrderKind>,
        min_expiry: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<Option<ethcontract::U256>> {
        let _timer = self
            .metrics
            .database_query_histogram("get_min_fee")
            .start_timer();
        self.inner
            .get_min_fee(sell_token, buy_token, amount, kind, min_expiry)
            .await
    }
}

#[async_trait::async_trait]
impl OrderStoring for Instrumented {
    async fn insert_order(
        &self,
        order: &model::order::Order,
    ) -> anyhow::Result<(), super::orders::InsertionError> {
        let _timer = self
            .metrics
            .database_query_histogram("insert_order")
            .start_timer();
        self.inner.insert_order(order).await
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

    fn orders<'a>(
        &'a self,
        filter: &'a super::orders::OrderFilter,
    ) -> futures::stream::BoxStream<'a, anyhow::Result<model::order::Order>> {
        self.inner.orders(filter)
    }
}

impl TradeRetrieving for Instrumented {
    fn trades<'a>(
        &'a self,
        filter: &'a super::trades::TradeFilter,
    ) -> futures::stream::BoxStream<'a, anyhow::Result<model::trade::Trade>> {
        self.inner.trades(filter)
    }
}

#[async_trait::async_trait]
impl Maintaining for Instrumented {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        let _timer = self
            .metrics
            .database_query_histogram("remove_expired_fee_measurements")
            .start_timer();
        self.inner.run_maintenance().await
    }
}
