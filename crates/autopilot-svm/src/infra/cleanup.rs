//! Periodic deletion of expired quotes and old order events.

use {
    crate::infra::{config::DbCleanup, db},
    sqlx::PgPool,
};

/// Deletes expired quotes and order events older than `cleanup_threshold`,
/// once at startup and then every `cleanup_interval`. A failed run is logged
/// and retried on the next tick.
pub fn spawn(pool: PgPool, config: DbCleanup) {
    let event_age =
        chrono::Duration::from_std(config.cleanup_threshold).expect("event age fits chrono");
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(config.cleanup_interval);
        loop {
            interval.tick().await;
            let now = chrono::Utc::now();
            match db::remove_expired_quotes(&pool, now).await {
                Ok(deleted) => tracing::debug!(deleted, "expired quotes cleanup"),
                Err(err) => tracing::warn!(?err, "failed to delete expired quotes"),
            }
            match db::delete_order_events_before(&pool, now - event_age).await {
                Ok(deleted) => tracing::debug!(deleted, "order events cleanup"),
                Err(err) => tracing::warn!(?err, "failed to delete old order events"),
            }
        }
    });
}
