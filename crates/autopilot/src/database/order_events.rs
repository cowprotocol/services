pub use database::order_events::OrderEventLabel;
use {
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
    /// Inserts the given events with the current timestamp into the DB.
    /// If this function encounters an error it will only be printed. More
    /// elaborate error handling is not necessary because this is just
    /// debugging information.
    pub async fn store_order_events(&self, events: &[(OrderUid, OrderEventLabel)]) {
        if let Err(err) = store_order_events(self, events, Utc::now()).await {
            tracing::warn!(?err, "failed to insert order events");
        }
    }

    /// Inserts an invalid order event for each order uid in the given set.
    /// Unique order uids are required to avoid inserting duplicated `Invalid`
    /// events for the same order.
    pub async fn store_invalid_order_events(&self, order_uids: &HashSet<OrderUid>) {
        if let Err(err) = store_invalid_order_events(self, order_uids, Utc::now()).await {
            tracing::warn!(?err, "failed to insert invalid order events");
        }
    }

    /// Deletes events before the provided timestamp.
    pub async fn delete_order_events_before(&self, timestamp: DateTime<Utc>) -> Result<u64, Error> {
        order_events::delete_order_events_before(&self.pool, timestamp).await
    }
}

async fn store_order_events(
    db: &super::Postgres,
    events: &[(OrderUid, OrderEventLabel)],
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

async fn store_invalid_order_events(
    db: &super::Postgres,
    order_uids: &HashSet<OrderUid>,
    timestamp: DateTime<Utc>,
) -> Result<()> {
    let mut ex = db.0.begin().await.context("begin transaction")?;
    for uid in order_uids {
        let event = OrderEvent {
            order_uid: ByteArray(uid.0),
            timestamp,
            label: OrderEventLabel::Invalid,
        };

        order_events::insert_invalid_order_event(&mut ex, &event).await?
    }
    ex.commit().await?;
    Ok(())
}
