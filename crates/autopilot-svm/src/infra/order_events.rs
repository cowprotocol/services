//! Order event writes: the per-order auction-progress log.

use {
    anyhow::{Context, Result},
    chain_types::solana::IntentHash,
    database::order_events::OrderEventLabel,
    sqlx::PgExecutor,
};

/// Append one event per order, skipping orders whose latest event already
/// carries the label, so a looping order marks each state once instead of
/// once per cycle. Best effort by design: callers log failures, a lost event
/// degrades the status endpoint, never the competition.
pub async fn store(
    ex: impl PgExecutor<'_>,
    uids: impl IntoIterator<Item = IntentHash>,
    label: OrderEventLabel,
) -> Result<()> {
    let uids: Vec<Vec<u8>> = uids.into_iter().map(|uid| uid.0.to_vec()).collect();
    if uids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"
WITH latest_events AS (
    SELECT DISTINCT ON (order_uid) order_uid, label
    FROM solana.order_events
    WHERE order_uid = ANY($1)
    ORDER BY order_uid, timestamp DESC
),
incoming AS (
    SELECT t.order_uid, now() AS timestamp, $2 AS label
    FROM unnest($1::bytea[]) AS t(order_uid)
)
INSERT INTO solana.order_events (order_uid, timestamp, label)
SELECT i.order_uid, i.timestamp, i.label
FROM incoming i
LEFT JOIN latest_events le ON le.order_uid = i.order_uid
WHERE le.label IS DISTINCT FROM i.label
        "#,
    )
    .bind(uids)
    .bind(label)
    .execute(ex)
    .await
    .context("insert solana.order_events")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::PgPool};

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_writes_order_events() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
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
