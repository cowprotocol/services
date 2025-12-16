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

/// Classifies order events as informational or error diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq, sqlx::Type)]
#[sqlx(type_name = "OrderEventType")]
#[sqlx(rename_all = "lowercase")]
pub enum OrderEventType {
    /// Informational diagnostic event
    Info,
    /// Error diagnostic event
    Error,
}

/// Contains a single event of the life cycle of an order and when it was
/// registered.
#[derive(Clone, Debug, Eq, PartialEq, sqlx::FromRow)]
pub struct OrderEvent {
    /// Which order this event belongs to
    pub order_uid: OrderUid,
    /// When the event was noticed and not necessarily when it was inserted into
    /// the DB
    pub timestamp: DateTime<Utc>,
    /// What kind of event happened
    pub label: OrderEventLabel,
    /// Optional event type for diagnostic events
    #[sqlx(rename = "type")]
    pub event_type: Option<OrderEventType>,
    /// Optional diagnostic message
    #[sqlx(rename = "message")]
    pub diag_message: Option<String>,
    /// Optional component identifier (e.g., 'autopilot', 'orderbook', 'driver')
    pub component: Option<String>,
}

/// Inserts a row into the `order_events` table only if the latest event for the
/// corresponding order UID has a different label than the provided event.
pub async fn insert_order_event(
    ex: &mut PgConnection,
    event: &OrderEvent,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        WITH cte AS (
            SELECT label, message, component
            FROM order_events
            WHERE order_uid = $1
            ORDER BY timestamp DESC
            LIMIT 1
        )
        INSERT INTO order_events (order_uid, timestamp, label, type, message, component)
        SELECT $1, $2, $3, $4, $5, $6
        WHERE NOT EXISTS (
            SELECT 1
            FROM cte
            WHERE label = $3 AND message = $5 AND component = $6
        )
    "#;
    sqlx::query(QUERY)
        .bind(event.order_uid)
        .bind(event.timestamp)
        .bind(event.label)
        .bind(event.event_type)
        .bind(&event.diag_message)
        .bind(&event.component)
        .execute(ex)
        .await
        .map(|_| ())
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

impl OrderEvent {
    /// Creates a lifecycle event.
    /// @param order_uid The order UID the event belongs to.
    /// @param label The status which has been assigned to the order due to this event
    /// @param timestamp The time the event occured.
    /// @param event_type Whether this situation is an error or just informational.
    /// @param diag_message A diagnostic message describing the situation.
    /// @param component The crate or subsystem where this event originates.
    pub fn new(
        order_uid: OrderUid,
        label: OrderEventLabel,
        event_type: OrderEventType,
        diag_message: String,
        component: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            order_uid,
            timestamp,
            label,
            event_type: Some(event_type),
            diag_message: Some(diag_message),
            component: Some(component),
        }
    }

    /// Creates a lifecycle event without the current timestamp.
    pub fn new_without_timestamp(
        order_uid: OrderUid,
        label: OrderEventLabel,
        event_type: OrderEventType,
        diag_message: String,
        component: String,
    ) -> Self {
        Self::new(order_uid, label, event_type, diag_message, component, Utc::now())
    }
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
        let event_a = OrderEvent::lifecycle(
            uid_a,
            OrderEventLabel::Created,
            now - chrono::Duration::milliseconds(300),
        );
        insert_order_event(&mut ex, &event_a).await.unwrap();
        let event_b = OrderEvent::lifecycle(
            uid_a,
            OrderEventLabel::Invalid,
            now - chrono::Duration::milliseconds(200),
        );
        insert_order_event(&mut ex, &event_b).await.unwrap();
        let event_c = OrderEvent::lifecycle(
            uid_b,
            OrderEventLabel::Invalid,
            now - chrono::Duration::milliseconds(100),
        );
        insert_order_event(&mut ex, &event_c).await.unwrap();
        let event_d = OrderEvent::lifecycle(
            uid_a,
            OrderEventLabel::Invalid,
            now,
        );
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
}
