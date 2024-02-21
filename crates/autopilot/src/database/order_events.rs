pub use database::order_events::OrderEventLabel;
use {
    crate::domain,
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        order_events::{self, OrderEvent},
    },
    sqlx::Error,
};

impl super::Postgres {
    /// Deletes events before the provided timestamp.
    pub async fn delete_order_events_before(&self, timestamp: DateTime<Utc>) -> Result<u64, Error> {
        order_events::delete_order_events_before(&self.pool, timestamp).await
    }
}

pub async fn store_order_events(
    db: &super::Postgres,
    order_uids: Vec<domain::OrderUid>,
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

        order_events::insert_order_event(&mut ex, &event).await?
    }
    ex.commit().await?;
    Ok(())
}
