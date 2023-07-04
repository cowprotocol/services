pub use database::order_events::OrderEventLabel;
use {
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
    /// background task to not block the thread.
    pub fn store_order_events(&self, uids: Vec<OrderUid>, label: OrderEventLabel) {
        if uids.is_empty() {
            return;
        }

        let now = Utc::now();
        let db = self.0.clone();
        tokio::task::spawn(
            async move {
                let mut ex = match db.acquire().await {
                    Ok(ex) => ex,
                    Err(err) => {
                        tracing::warn!(
                            ?label,
                            ?uids,
                            ?err,
                            "failed to acquire DB connection; could not store order events"
                        );
                        return;
                    }
                };
                store_order_events(&mut ex, &uids, now, label).await;
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
) {
    for uid in uids {
        if let Err(err) = order_events::insert_order_event(
            ex,
            &OrderEvent {
                order_uid: ByteArray(uid.0),
                timestamp,
                label,
            },
        )
        .await
        {
            tracing::warn!(?label, ?uid, ?err, "failed to store order event");
        }
    }
}
