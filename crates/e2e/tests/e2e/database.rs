//! This module contains helper functions to query the state of the database
//! during a test.

use {
    crate::setup::Db,
    database::{byte_array::ByteArray, order_events},
    model::order::OrderUid,
};

/// Returns all events of that order in the order they happend (old to new).
pub async fn events_of_order(db: &Db, uid: &OrderUid) -> Vec<order_events::OrderEvent> {
    const QUERY: &str = "SELECT * FROM order_events WHERE order_uid = $1 ORDER BY timestamp ASC";
    let mut db = db.acquire().await.unwrap();
    sqlx::query_as(QUERY)
        .bind(ByteArray(uid.0))
        .fetch_all(&mut db)
        .await
        .unwrap()
}
