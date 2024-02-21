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
            interval.tick().await;

            let timestamp: DateTime<Utc> = Utc::now() - self.config.event_age_threshold;
            match self.db.delete_order_events_before(timestamp).await {
                Ok(affected_rows_count) => {
                    tracing::debug!(affected_rows_count, timestamp = %timestamp.to_string(), "order events cleanup");
                    Metrics::get().order_events_cleanup_total.inc()
                }
                Err(err) => {
                    tracing::warn!(?err, "failed to delete order events before {}", timestamp)
                }
            }
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// The total number of successful `order_events` table cleanups
    #[metric(name = "periodic_db_cleanup")]
    order_events_cleanup_total: prometheus::IntCounter,
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
        itertools::Itertools,
        sqlx::{PgPool, Row},
    };

    // Note: `tokio::time::advance` was not used in these tests. While it is a
    // useful tool for controlling time flow in asynchronous tests, it causes
    // complications when used with `sqlx::PgPool`. Specifically, pausing or
    // advancing time with `tokio::time::advance` can interfere with the pool's
    // ability to acquire database connections, leading to panics and unpredictable
    // behavior in tests. Given these issues, tests were designed without
    // manipulating the timer, to maintain stability and reliability in the
    // database connection handling.
    #[tokio::test]
    #[ignore]
    async fn postgres_order_events_cleaner_flow() {
        let db = Postgres::with_defaults().await.unwrap();
        let mut ex = db.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();

        let now = Utc::now();
        let event_a = OrderEvent {
            order_uid: ByteArray([1; 56]),
            timestamp: now - chrono::Duration::milliseconds(300),
            label: OrderEventLabel::Created,
        };
        database::order_events::insert_order_event(&mut ex, &event_a)
            .await
            .unwrap();
        let event_b = OrderEvent {
            order_uid: ByteArray([2; 56]),
            timestamp: now - chrono::Duration::milliseconds(100),
            label: OrderEventLabel::Created,
        };
        database::order_events::insert_order_event(&mut ex, &event_b)
            .await
            .unwrap();
        let event_c = OrderEvent {
            order_uid: ByteArray([3; 56]),
            timestamp: now,
            label: OrderEventLabel::Created,
        };
        database::order_events::insert_order_event(&mut ex, &event_c)
            .await
            .unwrap();

        ex.commit().await.unwrap();

        let ids = order_event_ids_before(&db.pool).await;
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&event_a.order_uid));
        assert!(ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        let config =
            OrderEventsCleanerConfig::new(Duration::from_millis(50), Duration::from_millis(200));
        let cleaner = OrderEventsCleaner::new(config, db.clone());

        tokio::task::spawn(cleaner.run_forever());

        // delete `order_a` after the initialization
        time::sleep(Duration::from_millis(20)).await;
        let ids = order_event_ids_before(&db.pool).await;
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&event_a.order_uid));
        assert!(ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        // nothing deleted after the first interval
        time::sleep(Duration::from_millis(50)).await;
        let ids = order_event_ids_before(&db.pool).await;
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&event_a.order_uid));
        assert!(ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        // delete `event_b` only
        time::sleep(Duration::from_millis(100)).await;
        let ids = order_event_ids_before(&db.pool).await;
        assert_eq!(ids.len(), 1);
        assert!(!ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        // delete `event_c`
        time::sleep(Duration::from_millis(200)).await;
        let ids = order_event_ids_before(&db.pool).await;
        assert!(ids.is_empty());
    }

    async fn order_event_ids_before(pool: &PgPool) -> Vec<ByteArray<56>> {
        const QUERY: &str = r#"
                SELECT order_uid
                FROM order_events
            "#;
        sqlx::query(QUERY)
            .fetch_all(pool)
            .await
            .unwrap()
            .iter()
            .map(|row| {
                let order_uid: ByteArray<56> = row.try_get(0).unwrap();
                order_uid
            })
            .collect_vec()
    }
}
