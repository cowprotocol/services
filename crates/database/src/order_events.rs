//! Stores timestamped events of every order throughout its lifecycle.
//! This information gets used to compuate service level indicators.

use {
    crate::OrderUid,
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

/// Inserts a row into the `order_events` table.
pub async fn insert_order_event(
    ex: &mut PgConnection,
    event: &OrderEvent,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO order_events (
            order_uid,
            timestamp,
            label
        )
        VALUES ($1, $2, $3)
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
