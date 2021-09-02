use super::{orders::OrderStoring, trades::TradeRetrieving, Postgres};
use crate::fee::MinFeeStoring;
use prometheus::Histogram;
use shared::{event_handling::EventStoring, maintenance::Maintaining};
use tokio::time::Duration;

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Instrumented {
    inner: Postgres,
}

impl Instrumented {
    pub fn new(inner: Postgres) -> Self {
        Self { inner }
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
        let _guard = DatabaseMetrics::instance().on_request_start("replace_events");
        self.inner.replace_events(events, range).await
    }

    async fn append_events(
        &mut self,
        events: Vec<ethcontract::Event<contracts::gpv2_settlement::Event>>,
    ) -> anyhow::Result<()> {
        let _guard = DatabaseMetrics::instance().on_request_start("append_events");
        self.inner.append_events(events).await
    }

    async fn last_event_block(&self) -> anyhow::Result<u64> {
        let _guard = DatabaseMetrics::instance().on_request_start("last_event_block");
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
        let _guard = DatabaseMetrics::instance().on_request_start("save_fee_measurement");
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
        let _guard = DatabaseMetrics::instance().on_request_start("get_min_fee");
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
        let _guard = DatabaseMetrics::instance().on_request_start("insert_order");
        self.inner.insert_order(order).await
    }

    async fn cancel_order(
        &self,
        order_uid: &model::order::OrderUid,
        now: chrono::DateTime<chrono::Utc>,
    ) -> anyhow::Result<()> {
        let _guard = DatabaseMetrics::instance().on_request_start("cancel_order");
        self.inner.cancel_order(order_uid, now).await
    }

    async fn orders(
        &self,
        filter: &super::orders::OrderFilter,
    ) -> anyhow::Result<Vec<model::order::Order>> {
        let _guard = DatabaseMetrics::instance().on_request_start("orders");
        self.inner.orders(filter).await
    }

    async fn single_order(
        &self,
        uid: &model::order::OrderUid,
    ) -> anyhow::Result<Option<model::order::Order>> {
        let _guard = DatabaseMetrics::instance().on_request_start("single_order");
        self.inner.single_order(uid).await
    }

    async fn solvable_orders(&self, min_valid_to: u32) -> anyhow::Result<Vec<model::order::Order>> {
        let _guard = DatabaseMetrics::instance().on_request_start("solvable_orders");
        self.inner.solvable_orders(min_valid_to).await
    }

    async fn user_orders(
        &self,
        owner: &ethcontract::H160,
        offset: u64,
        limit: Option<u64>,
    ) -> anyhow::Result<Vec<model::order::Order>> {
        let _guard = DatabaseMetrics::instance().on_request_start("user_orders");
        self.inner.user_orders(owner, offset, limit).await
    }
}

#[async_trait::async_trait]
impl TradeRetrieving for Instrumented {
    async fn trades(
        &self,
        filter: &super::trades::TradeFilter,
    ) -> anyhow::Result<Vec<model::trade::Trade>> {
        let _guard = DatabaseMetrics::instance().on_request_start("trades");
        self.inner.trades(filter).await
    }
}

#[async_trait::async_trait]
impl Maintaining for Instrumented {
    async fn run_maintenance(&self) -> anyhow::Result<()> {
        let _guard =
            DatabaseMetrics::instance().on_request_start("remove_expired_fee_measurements");
        self.inner.run_maintenance().await
    }
}

pub async fn update_database_metrics(database: Postgres) -> ! {
    loop {
        match database.count_rows_in_tables().await {
            Ok(counts) => {
                let metrics = DatabaseMetrics::instance();
                for (table, count) in counts {
                    metrics.table_rows.with_label_values(&[table]).set(count);
                }
            }
            Err(err) => tracing::error!(?err, "failed to update db metrics"),
        };
        tokio::time::sleep(Duration::from_secs(10)).await;
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "database")]
struct DatabaseMetrics {
    /// Number of inflight database requests.
    #[metric(labels("query"))]
    requests_inflight: prometheus::IntGaugeVec,

    /// Number of completed database requests.
    #[metric(labels("query"))]
    requests_complete: prometheus::CounterVec,

    /// Execution time for each database request.
    #[metric(labels("query"))]
    requests_duration_seconds: prometheus::HistogramVec,

    /// Number of rows in db tables.
    #[metric(labels("table"))]
    table_rows: prometheus::IntGaugeVec,
}

impl DatabaseMetrics {
    fn instance() -> &'static Self {
        lazy_static::lazy_static! {
            static ref INSTANCE: DatabaseMetrics =
                DatabaseMetrics::new(shared::metrics::get_metrics_registry()).unwrap();
        }

        &INSTANCE
    }

    #[must_use]
    fn on_request_start(&self, query: &str) -> impl Drop {
        let requests_inflight = self.requests_inflight.with_label_values(&[query]);
        let requests_complete = self.requests_complete.with_label_values(&[query]);
        let requests_duration_seconds = self.requests_duration_seconds.with_label_values(&[query]);

        requests_inflight.inc();
        let timer = requests_duration_seconds.start_timer();

        scopeguard::guard(timer, move |timer| {
            requests_inflight.dec();
            requests_complete.inc();
            timer.stop_and_record();
        })
    }
}
