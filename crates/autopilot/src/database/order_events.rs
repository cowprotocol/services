pub use database::order_events::OrderEventLabel;
use {
    crate::domain,
    anyhow::Result,
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        order_events::{self, OrderEvent},
    },
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
    let events_count = order_uids.len();
    let mut ex = ex.begin().await.expect("begin transaction");
    for uid in order_uids {
        let event = OrderEvent {
            order_uid: ByteArray(uid.0),
            timestamp,
            label,
        };

        if let Err(err) = order_events::insert_order_event(&mut ex, &event).await {
            tracing::warn!(?err, ?events_count, "failed to insert order events");
            panic!("Failed to insert order events: {:?}", err);
        }
    }
    ex.commit().await.expect("commit transaction");
    tracing::debug!(elapsed=?start.elapsed(), ?events_count, "stored order events");
}
