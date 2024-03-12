//! Stores timestamped events of every order throughout its lifecycle.
//! This information gets used to compuate service level indicators.

use {
    crate::{byte_array::ByteArray, OrderUid},
    chrono::Utc,
    sqlx::{types::chrono::DateTime, PgConnection, PgPool},
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
    const QUERY: &str = r#"
        WITH cte AS (
            SELECT label
            FROM order_events
            WHERE order_uid = $1
            ORDER BY timestamp DESC
            LIMIT 1
        )
        INSERT INTO order_events (order_uid, timestamp, label)
        SELECT $1, $2, $3
        WHERE NOT EXISTS (
            SELECT 1
            FROM cte
            WHERE label = $3
        )
    "#;
    sqlx::query(QUERY)
        .bind(event.order_uid)
        .bind(event.timestamp)
        .bind(event.label)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Deletes rows before the provided timestamp from the `order_events` table.
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

pub async fn get_label(ex: &mut PgConnection, order: &OrderUid) -> Result<OrderEvent, sqlx::Error> {
    const QUERY: &str =
        r#"SELECT * FROM order_events WHERE order_uid = $1 ORDER BY timestamp DESC LIMIT 1"#;
    sqlx::query_as(QUERY)
        .bind(ByteArray(order.0))
        .fetch_one(ex)
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
}
