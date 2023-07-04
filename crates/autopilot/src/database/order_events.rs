pub use database::order_events::OrderEventLabel;
use {
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        order_events::{self, OrderEvent},
    },
    model::order::OrderUid,
    sqlx::PgConnection,
    tracing::Instrument,
};

impl super::Postgres {
    /// Stores the given order event for all passed [`OrderUid`]s in a
    /// background thread to not block the process.
    pub fn store_order_events(&self, uids: Vec<OrderUid>, label: OrderEventLabel) {
        if uids.is_empty() {
            return;
        }

        let now = Utc::now();
        let db = self.0.clone();
        let insert = async move {
            let mut ex = db.acquire().await.context("acquire")?;
            store_order_events(&mut ex, &uids, now, label)
                .await
                .context("store events")
        };
        tokio::task::spawn(
            async move {
                if let Err(err) = insert.await {
                    tracing::warn!(?label, ?err, "failed to store order events");
                }
            }
            .instrument(tracing::Span::current()),
        );
    }
}

async fn store_order_events(
    ex: &mut PgConnection,
    uids: &[OrderUid],
    timestamp: DateTime<Utc>,
    label: OrderEventLabel,
) -> Result<()> {
    for uid in uids {
        order_events::insert_order_event(
            ex,
            &OrderEvent {
                order_uid: ByteArray(uid.0),
                timestamp,
                label,
            },
        )
        .await?;
    }
    Ok(())
}
