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

#[cfg(test)]
mod tests {
    use {
        super::*,
        database::{
            byte_array::ByteArray,
            order_events::{OrderEvent, OrderEventLabel},
        },
        sqlx::PgConnection,
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_count_rows_in_table_() {
        let db = Postgres::new("postgresql://").await.unwrap();
        let mut ex = db.0.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();

        let event_a = OrderEvent {
            order_uid: ByteArray([1; 56]),
            timestamp: Utc::now() - chrono::Duration::days(31),
            label: OrderEventLabel::Created,
        };
        let latest_timestamp = Utc::now() - chrono::Duration::days(29);
        let event_b = OrderEvent {
            order_uid: ByteArray([2; 56]),
            timestamp: latest_timestamp,
            label: OrderEventLabel::Created,
        };

        database::order_events::insert_order_event(&mut ex, &event_a)
            .await
            .unwrap();
        database::order_events::insert_order_event(&mut ex, &event_b)
            .await
            .unwrap();

        let count = order_events_before_timestamp_count(
            &mut ex,
            latest_timestamp + chrono::Duration::days(1),
        )
        .await;
        assert_eq!(count, 2u64);

        let count = order_events_before_timestamp_count(&mut ex, latest_timestamp).await;
        assert_eq!(count, 1u64);

        async fn order_events_before_timestamp_count(
            ex: &mut PgConnection,
            timestamp: DateTime<Utc>,
        ) -> u64 {
            const QUERY: &str = r#"
                SELECT COUNT(1)
                FROM order_events
                WHERE timestamp < $1
                "#;
            let count: i64 = sqlx::query_scalar(QUERY)
                .bind(timestamp)
                .fetch_one(ex)
                .await
                .unwrap();

            count as u64
        }
    }
}
