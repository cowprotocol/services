//! The per-order auction-progress log.

use {
    anyhow::{Context, Result},
    chain_types::solana::IntentHash,
    database::solana::OrderEventLabel,
    sqlx::PgPool,
};

/// Advisory-lock namespace for the deduplicating insert, one lock per label.
const DEDUP_LOCK: &str = "solana_order_events_dedup";

/// Takes the [`DEDUP_LOCK`] of the bound label.
const DEDUP_LOCK_QUERY: &str =
    "SELECT pg_advisory_xact_lock(hashtextextended($1::text || $2::text, 0))";

/// Append one event per order, skipping a label the order's latest event
/// already carries: a looping order marks each state once, not once per cycle.
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
    // The dedup below compares against committed rows only, so writers that
    // overlap both append. Keyed per label because writers carrying different
    // labels append in either order for the same result, leaving only
    // same-label writers to serialize. Only writers taking this lock are
    // race-free.
    sqlx::query(DEDUP_LOCK_QUERY)
        .bind(DEDUP_LOCK)
        .bind(label)
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
    .bind(label)
    .execute(&mut *tx)
    .await
    .context("insert solana.order_events")?;
    tx.commit().await.context("commit order event write")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unserialized writers would append the same label twice: the dedup
    /// reads committed rows only.
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
        // A repeated label is not appended.
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
