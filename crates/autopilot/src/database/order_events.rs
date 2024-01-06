pub use database::order_events::OrderEventLabel;
use {
    crate::domain,
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        order_events::{self, OrderEvent},
    },
    model::order::OrderUid,
    sqlx::Error,
    std::collections::HashSet,
};

impl super::Postgres {
    /// Inserts an order event for each order uid in the given set.
    /// Unique order uids are required to avoid inserting events with the same
    /// label within the same order_uid.
    pub async fn store_non_subsequent_label_order_events(
        &self,
        order_uids: &HashSet<OrderUid>,
        label: OrderEventLabel,
    ) {
        if let Err(err) =
            store_non_subsequent_label_order_events(self, order_uids, label, Utc::now()).await
        {
            tracing::warn!(?err, "failed to insert non-subsequent label order events");
        }
    }

    /// Deletes events before the provided timestamp.
    pub async fn delete_order_events_before(&self, timestamp: DateTime<Utc>) -> Result<u64, Error> {
        order_events::delete_order_events_before(&self.pool, timestamp).await
    }
}

pub async fn store_order_events(
    db: &super::Postgres,
    events: &[(domain::OrderUid, OrderEventLabel)],
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let mut ex = db.pool.begin().await.context("begin transaction")?;
    for chunk in events.chunks(db.config.order_events_insert_batch_size.get()) {
        let batch = chunk.iter().map(|(uid, label)| OrderEvent {
            order_uid: ByteArray(uid.0),
            timestamp,
            label: *label,
        });

        order_events::insert_order_events_batch(&mut ex, batch).await?
    }
    ex.commit().await?;
    Ok(())
}

async fn store_non_subsequent_label_order_events(
    db: &super::Postgres,
    order_uids: &HashSet<OrderUid>,
    label: OrderEventLabel,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let mut ex = db.pool.begin().await.context("begin transaction")?;
    for uid in order_uids {
        let event = OrderEvent {
            order_uid: ByteArray(uid.0),
            timestamp,
            label,
        };

        order_events::insert_non_subsequent_label_order_event(&mut ex, &event).await?
    }
    ex.commit().await?;
    Ok(())
}
