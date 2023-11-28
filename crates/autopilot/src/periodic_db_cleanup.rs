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
        println!("starting");
        interval.tick().await; // Initial tick to start the interval

        loop {
            let timestamp: DateTime<Utc> = Utc::now() - self.config.event_age_threshold;
            println!("removing before {}", timestamp);
            match self.db.delete_order_events_before(timestamp).await {
                Ok(affected_rows_count) => {
                    tracing::debug!(affected_rows_count, timestamp = %timestamp.to_string(), "deleted order events");
                    println!("removed {}", affected_rows_count);
                    Metrics::get().order_events_cleanup_total.inc()
                }
                Err(err) => {
                    println!("failed to remove {}", err);
                    tracing::warn!(?err, "failed to delete order events before {}", timestamp)
                }
            }
            println!("loop done");
            interval.tick().await;
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
        sqlx::{PgConnection, Row},
    };

    #[tokio::test]
    #[ignore]
    async fn order_events_cleaner_flow() {
        let db = Postgres::new("postgresql://").await.unwrap();
        let mut ex = db.0.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();

        let now = Utc::now();
        let event_a = OrderEvent {
            order_uid: ByteArray([1; 56]),
            timestamp: now - chrono::Duration::minutes(2),
            label: OrderEventLabel::Created,
        };
        let event_b = OrderEvent {
            order_uid: ByteArray([2; 56]),
            timestamp: now - chrono::Duration::minutes(1),
            label: OrderEventLabel::Created,
        };
        let event_c = OrderEvent {
            order_uid: ByteArray([3; 56]),
            timestamp: now,
            label: OrderEventLabel::Created,
        };

        database::order_events::insert_order_event(&mut ex, &event_a)
            .await
            .unwrap();
        database::order_events::insert_order_event(&mut ex, &event_b)
            .await
            .unwrap();
        database::order_events::insert_order_event(&mut ex, &event_c)
            .await
            .unwrap();

        let ids = order_event_ids_before(&mut ex, Utc::now()).await;
        assert_eq!(ids.len(), 3);
        assert!(ids.contains(&event_a.order_uid));
        assert!(ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        let config =
            OrderEventsCleanerConfig::new(Duration::from_secs(15), Duration::from_secs(90));
        let cleaner = OrderEventsCleaner::new(config, db);

        time::pause();
        tokio::task::spawn(cleaner.run_forever());

        // deleted `order_a` only right after the initialization
        time::advance(Duration::from_secs(1)).await;
        let ids = order_event_ids_before(&mut ex, Utc::now()).await;
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&event_a.order_uid));
        assert!(ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        // nothing deleted after the first interval
        time::advance(Duration::from_secs(15)).await;
        let ids = order_event_ids_before(&mut ex, Utc::now()).await;
        assert_eq!(ids.len(), 2);
        assert!(!ids.contains(&event_a.order_uid));
        assert!(ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        // deleted `event_b` only
        time::advance(Duration::from_secs(15)).await;
        let ids = order_event_ids_before(&mut ex, Utc::now()).await;
        assert_eq!(ids.len(), 1);
        assert!(!ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));

        // deleted `event_c` only
        time::advance(Duration::from_secs(60)).await;
        let ids = order_event_ids_before(&mut ex, Utc::now()).await;
        assert_eq!(ids.len(), 1);
        assert!(!ids.contains(&event_b.order_uid));
        assert!(ids.contains(&event_c.order_uid));
    }

    async fn order_event_ids_before(
        ex: &mut PgConnection,
        timestamp: DateTime<Utc>,
    ) -> Vec<ByteArray<56>> {
        const QUERY: &str = r#"
                SELECT order_uid, timestamp
                FROM order_events
                WHERE timestamp < $1
            "#;
        sqlx::query(QUERY)
            .bind(timestamp)
            .fetch_all(ex)
            .await
            .unwrap()
            .iter()
            .map(|row| {
                let timestamp: DateTime<Utc> = row.try_get(1).unwrap();
                let order_uid: ByteArray<56> = row.try_get(0).unwrap();
                println!("timestamp={}, order_uid={:?}", timestamp, order_uid);
                order_uid
            })
            .collect_vec()
    }
}
