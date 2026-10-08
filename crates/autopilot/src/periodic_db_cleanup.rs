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
            self.cleanup_once(Utc::now()).await;
        }
    }

    async fn cleanup_once(&self, now: DateTime<Utc>) {
        let timestamp = now - self.config.event_age_threshold;
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

    #[tokio::test]
    #[ignore]
    async fn postgres_order_events_cleaner_flow() {
        let db = Postgres::with_defaults().await.unwrap();
        let mut ex = db.pool.begin().await.unwrap();
        database::clear_DANGER_(&mut ex).await.unwrap();

        let t0 = Utc::now();
        let event_at = |uid: u8, offset_ms: i64| OrderEvent {
            order_uid: ByteArray([uid; 56]),
            timestamp: t0 + chrono::Duration::milliseconds(offset_ms),
            label: OrderEventLabel::Created,
            reason: None,
        };
        let event_a = event_at(1, 0);
        let event_b = event_at(2, 200);
        let event_c = event_at(3, 400);
        for event in [&event_a, &event_b, &event_c] {
            database::order_events::insert_order_event(&mut ex, event)
                .await
                .unwrap();
        }
        ex.commit().await.unwrap();

        let ids = order_event_ids(&db.pool).await;
        assert_eq!(ids.len(), 3);

        let threshold_ms: i64 = 100;
        let cleaner = OrderEventsCleaner::new(
            OrderEventsCleanerConfig::new(
                Duration::from_secs(60),
                Duration::from_millis(threshold_ms as u64),
            ),
            db.clone(),
        );
        let tick_at = async |offset_ms: i64| {
            cleaner
                .cleanup_once(t0 + chrono::Duration::milliseconds(offset_ms))
                .await;
            order_event_ids(&db.pool).await
        };

        // now=t0+150ms, cutoff=t0+50ms: deletes event_a (at t0).
        let ids = tick_at(threshold_ms + 50).await;
        assert_eq!(ids, vec![event_b.order_uid, event_c.order_uid]);

        // now=t0+250ms, cutoff=t0+150ms: event_b (at t0+200ms) still safe.
        let ids = tick_at(threshold_ms + 150).await;
        assert_eq!(ids, vec![event_b.order_uid, event_c.order_uid]);

        // now=t0+350ms, cutoff=t0+250ms: deletes event_b.
        let ids = tick_at(threshold_ms + 250).await;
        assert_eq!(ids, vec![event_c.order_uid]);

        // now=t0+550ms, cutoff=t0+450ms: deletes event_c.
        let ids = tick_at(threshold_ms + 450).await;
        assert!(ids.is_empty());
    }

    async fn order_event_ids(pool: &PgPool) -> Vec<ByteArray<56>> {
        const QUERY: &str = r#"
                SELECT order_uid
                FROM order_events
                ORDER BY timestamp
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
