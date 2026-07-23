pub use database::order_events::{OrderEventLabel, OrderFilterReason};
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

/// Max number of order UIDs sent to the DB per insert statement. Chunking keeps
/// individual queries bounded regardless of how many events are stored at once.
const INSERT_CHUNK_SIZE: usize = 1000;

pub async fn store_order_events(
    ex: &mut PgConnection,
    order_uids: impl IntoIterator<Item = domain::OrderUid>,
    label: OrderEventLabel,
    reason: Option<OrderFilterReason>,
    timestamp: DateTime<Utc>,
) {
    let start = Instant::now();

    let insert = async move {
        let mut ex = ex.begin().await?;
        // Map lazily and drain the iterator one chunk at a time so we never
        // materialize the full (potentially thousands of entries) UID list.
        // NOTE: `itertools::chunks` can't be used here because its lazy groups
        // borrow a `!Sync` handle across the `.await`, which would make this
        // spawned future non-`Send`.
        let mut order_uids = order_uids.into_iter().map(|o| ByteArray(o.0));
        let mut chunk = Vec::with_capacity(INSERT_CHUNK_SIZE);
        let mut count = 0;
        loop {
            chunk.clear();
            chunk.extend(order_uids.by_ref().take(INSERT_CHUNK_SIZE));
            if chunk.is_empty() {
                break;
            }
            count += chunk.len();
            order_events::insert_order_events(&mut ex, &chunk, timestamp, label, reason).await?;
        }
        ex.commit().await?;
        Ok::<_, Error>(count)
    };

    match insert.await {
        Ok(count) => {
            tracing::debug!(?label, count, elapsed = ?start.elapsed(), "stored order events")
        }
        Err(err) => tracing::warn!(?label, ?err, "failed to insert order events"),
    }
}
