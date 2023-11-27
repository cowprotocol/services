use {
    crate::database::Postgres,
    chrono::{DateTime, Utc},
    std::time::Duration,
    tokio::time,
};

pub struct OrderEventsCleanerConfig {
    cleanup_interval: Duration,
    event_age_threshold: chrono::Duration,
}

impl OrderEventsCleanerConfig {
    pub fn new(cleanup_interval: Duration, event_age_threshold: Duration) -> Self {
        OrderEventsCleanerConfig {
            cleanup_interval,
            event_age_threshold: chrono::Duration::from_std(event_age_threshold).unwrap(),
        }
    }
}

pub struct OrderEventsCleaner {
    config: OrderEventsCleanerConfig,
    db: Postgres,
}

impl OrderEventsCleaner {
    pub fn new(config: OrderEventsCleanerConfig, db: Postgres) -> Self {
        OrderEventsCleaner { config, db }
    }

    pub async fn run_forever(self) -> ! {
        let mut interval = time::interval(self.config.cleanup_interval);
        loop {
            let timestamp: DateTime<Utc> = Utc::now() - self.config.event_age_threshold;
            match self.db.delete_events_before(timestamp).await {
                Ok(affected_rows_count) => {
                    tracing::debug!(
                        "deleted {:?} order events before {}",
                        affected_rows_count,
                        timestamp
                    );
                    Metrics::get()
                        .last_order_events_cleanup_run
                        .set(Utc::now().timestamp())
                }
                Err(err) => {
                    tracing::warn!(?err, "failed to delete order events before {}", timestamp)
                }
            }
            interval.tick().await;
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Timestamp of the last successful `order_events` table cleanup.
    #[metric(name = "periodic_db_cleanup", labels("type"))]
    last_order_events_cleanup_run: prometheus::IntGauge,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}
