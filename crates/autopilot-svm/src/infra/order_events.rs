//! Order event writes: the per-order auction-progress log.

use {
    anyhow::{Context, Result},
    chain_types::solana::IntentHash,
    database::order_events::OrderEventLabel,
    sqlx::PgPool,
};

/// Advisory-lock name serializing the deduplicating insert.
const DEDUP_LOCK: &str = "solana_order_events_dedup";

/// Append one event per order, skipping orders whose latest event already
/// carries the label, so a looping order marks each state once instead of
/// once per cycle. Best effort by design: callers log failures, a lost event
/// degrades the status endpoint, never the competition.
pub async fn store(
    pool: &PgPool,
    uids: impl IntoIterator<Item = IntentHash>,
    label: OrderEventLabel,
) -> Result<()> {
    let uids: Vec<Vec<u8>> = uids.into_iter().map(|uid| uid.0.to_vec()).collect();
    if uids.is_empty() {
        return Ok(());
    }
    let mut tx = pool.begin().await.context("begin order event write")?;
    // The insert skips an event that repeats the order's latest label, a
    // comparison that only sees committed rows, so two overlapping writers
    // would both append. One lock for the whole table rather than per order:
    // these transactions are small and run detached. Writers that insert
    // events without taking this lock stay exposed to the same race.
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(DEDUP_LOCK)
        .execute(&mut *tx)
        .await
        .context("lock solana.order_events")?;
    sqlx::query(
        r#"
WITH latest_events AS (
    SELECT DISTINCT ON (order_uid) order_uid, label
    FROM solana.order_events
    WHERE order_uid = ANY($1)
    ORDER BY order_uid, timestamp DESC
),
incoming AS (
    SELECT t.order_uid, now() AS timestamp, $2::solana.OrderEventLabel AS label
    FROM unnest($1::bytea[]) AS t(order_uid)
)
INSERT INTO solana.order_events (order_uid, timestamp, label)
SELECT DISTINCT i.order_uid, i.timestamp, i.label
FROM incoming i
LEFT JOIN latest_events le ON le.order_uid = i.order_uid
WHERE le.label IS DISTINCT FROM i.label
        "#,
    )
    .bind(uids)
    .bind(label_str(label))
    .execute(&mut *tx)
    .await
    .context("insert solana.order_events")?;
    tx.commit().await.context("commit order event write")?;
    Ok(())
}

/// The wire value of a label, bound as text and cast in SQL: sqlx resolves a
/// derived enum's type by unqualified name, which is ambiguous on a database
/// holding both the base OrderEventLabel and the solana one.
fn label_str(label: OrderEventLabel) -> &'static str {
    match label {
        OrderEventLabel::Created => "created",
        OrderEventLabel::Ready => "ready",
        OrderEventLabel::Filtered => "filtered",
        OrderEventLabel::Invalid => "invalid",
        OrderEventLabel::Executing => "executing",
        OrderEventLabel::Considered => "considered",
        OrderEventLabel::Traded => "traded",
        OrderEventLabel::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Concurrent writers reporting the same label append one event: the
    /// insert compares against the order's latest event, which only holds
    /// while the writes are serialized.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_concurrent_writes_append_one_event() {
        let pool = crate::test_db::pool().await;
        sqlx::query("TRUNCATE solana.order_events")
            .execute(&pool)
            .await
            .unwrap();
        let uid = IntentHash([0x33; 32]);
        store(&pool, [uid], OrderEventLabel::Ready).await.unwrap();

        let write = || store(&pool, [uid], OrderEventLabel::Executing);
        let (first, second) = tokio::join!(write(), write());
        first.unwrap();
        second.unwrap();

        let labels: Vec<String> = sqlx::query_scalar(
            "SELECT label::text FROM solana.order_events WHERE order_uid = $1 ORDER BY timestamp",
        )
        .bind(uid.0.to_vec())
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(labels, ["ready", "executing"]);
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_writes_order_events() {
        let pool = crate::test_db::pool().await;
        sqlx::query("TRUNCATE solana.order_events")
            .execute(&pool)
            .await
            .unwrap();

        store(&pool, [], OrderEventLabel::Ready).await.unwrap();
        store(
            &pool,
            [IntentHash([0x11; 32]), IntentHash([0x22; 32])],
            OrderEventLabel::Ready,
        )
        .await
        .unwrap();
        // A repeated label on the same order is not appended again.
        store(&pool, [IntentHash([0x11; 32])], OrderEventLabel::Ready)
            .await
            .unwrap();
        store(&pool, [IntentHash([0x11; 32])], OrderEventLabel::Executing)
            .await
            .unwrap();

        let labels: Vec<String> = sqlx::query_scalar(
            "SELECT label::text FROM solana.order_events WHERE order_uid = $1 ORDER BY timestamp, \
             label",
        )
        .bind([0x11u8; 32].to_vec())
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(labels, ["ready", "executing"]);
    }
}
