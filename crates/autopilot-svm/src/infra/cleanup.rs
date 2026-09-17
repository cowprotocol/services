//! Periodic deletion of database rows nothing reads anymore.

use {crate::infra::db, sqlx::PgPool, std::time::Duration};

/// Deletes expired quotes and old order events on a fixed interval, like the
/// EVM autopilot's database cleanup.
pub struct Cleanup {
    pool: PgPool,
    interval: Duration,
    event_age: chrono::Duration,
}

impl Cleanup {
    pub fn new(pool: PgPool, interval: Duration, event_age: Duration) -> Self {
        Self {
            pool,
            interval,
            event_age: chrono::Duration::from_std(event_age).expect("event age fits chrono"),
        }
    }

    /// Run the cleanup forever. A failed run is logged and retried on the
    /// next tick.
    pub async fn run_forever(self) -> ! {
        let mut interval = tokio::time::interval(self.interval);
        loop {
            interval.tick().await;
            let now = chrono::Utc::now();
            match db::remove_expired_quotes(&self.pool, now).await {
                Ok(removed) => tracing::debug!(removed, "expired quotes cleanup"),
                Err(err) => tracing::warn!(?err, "failed to delete expired quotes"),
            }
            match db::remove_order_events_before(&self.pool, now - self.event_age).await {
                Ok(removed) => tracing::debug!(removed, "order events cleanup"),
                Err(err) => tracing::warn!(?err, "failed to delete old order events"),
            }
        }
    }
}
