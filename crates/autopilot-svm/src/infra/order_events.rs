//! Order event writes: the per-order auction-progress log.

use {
    anyhow::{Context, Result},
    chain_types::solana::IntentHash,
    database::order_events::OrderEventLabel,
    sqlx::PgExecutor,
};

/// Append one event per order. Best effort by design: callers log failures,
/// a lost event degrades the status endpoint, never the competition.
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
INSERT INTO solana.order_events (order_uid, timestamp, label)
SELECT uid, now(), $2 FROM UNNEST($1::bytea[]) AS uid
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
