//! Stores timestamped events of every order throughout its lifecycle.
//! This information gets used to compuate service level indicators.

use {
    crate::{OrderUid, byte_array::ByteArray},
    chrono::Utc,
    sqlx::{PgConnection, PgPool, types::chrono::DateTime},
    tracing::instrument,
};

/// Describes what kind of event was registered for an order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "OrderEventLabel")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderEventLabel {
    /// Order was added to the orderbook.
    Created,
    /// Order was included in an auction and got sent to the solvers.
    Ready,
    /// Order was filtered from the auction and did not get sent to the solvers.
    Filtered,
    /// Order can not be settled on-chain. (e.g. user is missing funds,
    /// PreSign or EIP-1271 signature is invalid, etc.)
    Invalid,
    /// Order was included in the winning settlement and is in the process of
    /// being submitted on-chain.
    Executing,
    /// Order was included in a valid settlement.
    Considered,
    /// Order was settled on-chain.
    Traded,
    /// Order was cancelled by the user.
    Cancelled,
}

/// Contains a single event of the life cycle of an order and when it was
/// registered.
#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type, sqlx::FromRow)]
pub struct OrderEvent {
    /// Which order this event belongs to
    pub order_uid: OrderUid,
    /// When the event was noticed and not necessarily when it was inserted into
    /// the DB
    pub timestamp: DateTime<Utc>,
    /// What kind of event happened
    pub label: OrderEventLabel,
}

/// Inserts a row into the `order_events` table only if the latest event for the
/// corresponding order UID has a different label than the provided event..
pub async fn insert_order_event(
    ex: &mut PgConnection,
    event: &OrderEvent,
) -> Result<(), sqlx::Error> {
    insert_order_events(ex, &[event.order_uid], event.timestamp, event.label).await
}

pub async fn insert_order_events(
    ex: &mut PgConnection,
    orders: &[OrderUid],
    timestamp: DateTime<Utc>,
    label: OrderEventLabel,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
    WITH latest_events AS (
        SELECT DISTINCT ON (order_uid) order_uid, label
        FROM order_events
        WHERE order_uid = ANY($1)
        ORDER BY order_uid, timestamp DESC
    ),
    incoming AS (
        SELECT t.order_uid, $2 AS timestamp, $3 AS label
        FROM unnest($1) AS t(order_uid)
    )
    INSERT INTO order_events (order_uid, timestamp, label)
    SELECT i.order_uid, i.timestamp, i.label
    FROM incoming i
    LEFT JOIN latest_events le ON le.order_uid = i.order_uid
    WHERE le.label IS DISTINCT FROM i.label
    "#;

    sqlx::query(QUERY)
        .bind(orders)
        .bind(timestamp)
        .bind(label)
        .execute(ex)
        .await?;
    Ok(())
}

/// Deletes rows before the provided timestamp from the `order_events` table.
#[instrument(skip_all)]
pub async fn delete_order_events_before(
    pool: &PgPool,
    timestamp: DateTime<Utc>,
) -> Result<u64, sqlx::Error> {
    const QUERY: &str = r#"
        DELETE FROM order_events
        WHERE timestamp < $1
    "#;
    sqlx::query(QUERY)
        .bind(timestamp)
        .execute(pool)
        .await
        .map(|result| result.rows_affected())
}

#[instrument(skip_all)]
pub async fn get_latest(
    ex: &mut PgConnection,
    order: &OrderUid,
) -> Result<Option<OrderEvent>, sqlx::Error> {
    const QUERY: &str =
        r#"SELECT * FROM order_events WHERE order_uid = $1 ORDER BY timestamp DESC LIMIT 1"#;
    sqlx::query_as(QUERY)
        .bind(ByteArray(order.0))
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            order_events::{OrderEvent, OrderEventLabel},
        },
        chrono::TimeZone,
        sqlx::Connection,
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_non_subsequent_order_events() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut ex = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut ex).await.unwrap();

        let now = Utc::now();
        let uid_a = ByteArray([1; 56]);
        let uid_b = ByteArray([2; 56]);
        let event_a = OrderEvent {
            order_uid: uid_a,
            timestamp: now - chrono::Duration::milliseconds(300),
            label: OrderEventLabel::Created,
        };
        insert_order_event(&mut ex, &event_a).await.unwrap();
        let event_b = OrderEvent {
            order_uid: uid_a,
            timestamp: now - chrono::Duration::milliseconds(200),
            label: OrderEventLabel::Invalid,
        };
        insert_order_event(&mut ex, &event_b).await.unwrap();
        let event_c = OrderEvent {
            order_uid: uid_b,
            timestamp: now - chrono::Duration::milliseconds(100),
            label: OrderEventLabel::Invalid,
        };
        insert_order_event(&mut ex, &event_c).await.unwrap();
        let event_d = OrderEvent {
            order_uid: uid_a,
            timestamp: now,
            label: OrderEventLabel::Invalid,
        };
        insert_order_event(&mut ex, &event_d).await.unwrap();

        ex.commit().await.unwrap();

        let ids = all_order_events(&mut db).await;

        assert_eq!(ids.len(), 3);
        assert_eq!(ids[0].order_uid, uid_a);
        assert_eq!(ids[0].label, OrderEventLabel::Created);
        assert_eq!(ids[1].order_uid, uid_a);
        assert_eq!(ids[1].label, OrderEventLabel::Invalid);
        assert_eq!(ids[2].order_uid, uid_b);
        assert_eq!(ids[2].label, OrderEventLabel::Invalid);

        let latest = get_latest(&mut db, &uid_a).await.unwrap().unwrap();
        assert_eq!(latest.order_uid, event_b.order_uid);
        assert_eq!(latest.label, event_b.label);
        // Postgres returns micros only while DateTime has nanos.
        assert_eq!(
            latest.timestamp.timestamp_micros(),
            event_b.timestamp.timestamp_micros()
        );
    }

    async fn all_order_events(ex: &mut PgConnection) -> Vec<OrderEvent> {
        const QUERY: &str = r#"
                SELECT *
                FROM order_events
                ORDER BY timestamp
            "#;
        sqlx::query_as::<_, OrderEvent>(QUERY)
            .fetch_all(ex)
            .await
            .unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_multi_insert() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut ex = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut ex).await.unwrap();

        let early = Utc.with_ymd_and_hms(2026, 2, 19, 0, 0, 0).unwrap();
        let uid_a = ByteArray([1; 56]);
        let uid_b = ByteArray([2; 56]);
        let event_a = OrderEvent {
            order_uid: uid_a,
            timestamp: early,
            label: OrderEventLabel::Created,
        };
        insert_order_event(&mut ex, &event_a).await.unwrap();
        let event_b = OrderEvent {
            order_uid: uid_b,
            timestamp: early,
            label: OrderEventLabel::Invalid,
        };
        insert_order_event(&mut ex, &event_b).await.unwrap();

        let later = Utc.with_ymd_and_hms(2027, 2, 19, 0, 0, 0).unwrap();
        insert_order_events(&mut ex, &[uid_a, uid_b], later, OrderEventLabel::Invalid)
            .await
            .unwrap();

        let a = get_latest(&mut ex, &uid_a).await.unwrap();
        assert_eq!(
            a,
            Some(OrderEvent {
                order_uid: uid_a,
                // new latest event was added
                timestamp: later,
                label: OrderEventLabel::Invalid,
            })
        );

        let b = get_latest(&mut ex, &uid_b).await.unwrap();
        assert_eq!(
            b,
            Some(OrderEvent {
                order_uid: uid_b,
                // since the latest event was already `Invalid`
                // no new entry was created
                timestamp: early,
                label: OrderEventLabel::Invalid
            })
        );
    }
}
