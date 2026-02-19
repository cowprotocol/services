pub use database::order_events::OrderEventLabel;
use {
    crate::domain,
    anyhow::Result,
    chrono::{DateTime, Utc},
    database::{byte_array::ByteArray, order_events},
    sqlx::{Acquire, Error, PgConnection},
    tokio::time::Instant,
};

impl super::Postgres {
    /// Deletes events before the provided timestamp.
    pub async fn delete_order_events_before(&self, timestamp: DateTime<Utc>) -> Result<u64, Error> {
        order_events::delete_order_events_before(&self.pool, timestamp).await
    }
}

pub async fn store_order_events(
    ex: &mut PgConnection,
    order_uids: Vec<domain::OrderUid>,
    label: OrderEventLabel,
    timestamp: DateTime<Utc>,
) {
    let start = Instant::now();
    let order_uids: Vec<_> = order_uids.into_iter().map(|o| ByteArray(o.0)).collect();
    let count = order_uids.len();

    let insert = async move {
        let mut ex = ex.begin().await?;
        for chunk in order_uids.chunks(100) {
            order_events::insert_order_events(&mut ex, chunk, timestamp, label).await?;
        }
        ex.commit().await
    };

    match insert.await {
        Ok(_) => tracing::debug!(?label, count, elapsed = ?start.elapsed(), "stored order events"),
        Err(err) => tracing::warn!(?label, count, ?err, "failed to insert order events"),
    }
}
