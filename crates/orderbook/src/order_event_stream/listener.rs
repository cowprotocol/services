use {
    super::OrderEventStream,
    crate::dto::order_event::OrderEvent,
    anyhow::{Context, Result},
    sqlx::{PgPool, postgres::PgListener},
    std::{sync::Arc, time::Duration},
};

/// Channel of the `order_event_insert_notify` database trigger. The payload
/// is an [`OrderEvent`] as JSON.
const CHANNEL: &str = "order_event";

const RECONNECT_DELAY: Duration = Duration::from_secs(1);

/// Publishes every order event the database notifies of, for as long as the
/// process runs.
pub fn spawn_notification_listener(order_event_stream: Arc<OrderEventStream>, pool: PgPool) {
    tokio::spawn(async move {
        loop {
            match publish_notifications(&order_event_stream, &pool).await {
                Ok(()) => tracing::warn!("lost the order event listener connection"),
                Err(err) => tracing::error!(?err, "order event listener failed"),
            }
            tokio::time::sleep(RECONNECT_DELAY).await;
        }
    });
}

/// Publishes notifications until the connection is lost.
async fn publish_notifications(order_event_stream: &OrderEventStream, pool: &PgPool) -> Result<()> {
    let mut listener = PgListener::connect_with(pool)
        .await
        .context("connect listener")?;
    listener.listen(CHANNEL).await.context("listen")?;
    tracing::info!("listening for order events");

    // Events from before this connection listened were not received. The
    // subscriptions must end only now: a client that reconnected earlier
    // would load its order and then still miss the events up to this point.
    order_event_stream.end_subscriptions();

    // `try_recv` reconnects on its next call. The connection is replaced
    // instead, because the subscriptions have to end after the new `LISTEN`.
    while let Some(notification) = listener.try_recv().await.context("receive")? {
        match serde_json::from_str::<OrderEvent>(notification.payload()) {
            Ok(event) => order_event_stream.publish(event),
            Err(err) => tracing::error!(
                ?err,
                payload = notification.payload(),
                "malformed order event notification"
            ),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::dto::order_event::{Label, Reason},
        chrono::{TimeZone, Utc},
        database::{
            byte_array::ByteArray,
            order_events::{OrderEventLabel, OrderFilterReason, insert_order_events},
        },
        model::order::OrderUid,
    };

    #[test]
    fn parses_notification() {
        let uid = OrderUid::from_integer(7);
        let payload = format!(
            r#"{{"uid" : "{uid}", "timestamp" : "2026-11-19T10:20:30.123456+00:00", "label" : "filtered", "reason" : "in_flight"}}"#
        );
        assert_eq!(
            serde_json::from_str::<OrderEvent>(&payload).unwrap(),
            OrderEvent {
                uid,
                timestamp: Utc.timestamp_micros(1_795_083_630_123_456).unwrap(),
                label: Label::Filtered,
                reason: Some(Reason::InFlight),
            }
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_trigger_notifies_of_inserted_events() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let mut listener = PgListener::connect_with(&pool).await.unwrap();
        listener.listen(CHANNEL).await.unwrap();

        let uid = OrderUid::from_integer(rand::random());
        let timestamp = Utc.timestamp_micros(1_795_083_630_123_456).unwrap();
        let mut ex = pool.acquire().await.unwrap();
        for (label, reason) in [
            (OrderEventLabel::Created, None),
            (
                OrderEventLabel::Invalid,
                Some(OrderFilterReason::InsufficientBalance),
            ),
        ] {
            insert_order_events(&mut ex, &[ByteArray(uid.0)], timestamp, label, reason)
                .await
                .unwrap();
        }

        let mut received = Vec::new();
        while received.len() < 2 {
            let notification = tokio::time::timeout(Duration::from_secs(5), listener.recv())
                .await
                .expect("no notification")
                .unwrap();
            let event: OrderEvent = serde_json::from_str(notification.payload()).unwrap();
            // Other tests can insert events at the same time.
            if event.uid == uid {
                received.push(event);
            }
        }

        let event = |label, reason| OrderEvent {
            uid,
            timestamp,
            label,
            reason,
        };
        assert_eq!(
            received,
            [
                event(Label::Created, None),
                event(Label::Invalid, Some(Reason::InsufficientBalance)),
            ]
        );
    }
}
