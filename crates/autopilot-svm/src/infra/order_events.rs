//! Order event writes: the auction-progress log behind the orderbook's
//! status endpoint.

use {
    anyhow::{Context, Result},
    chain_types::solana::IntentHash,
    sqlx::PgExecutor,
};

/// Auction-progress labels this autopilot reports.
#[derive(Debug, Clone, Copy)]
pub enum Label {
    /// The order entered the auction sent to the solvers.
    Ready,
    /// The order appeared only in ranked non-winning solutions.
    Considered,
    /// The order is part of a winning solution being submitted.
    Executing,
    /// The order's settlement was observed on chain.
    Traded,
}

impl Label {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Considered => "considered",
            Self::Executing => "executing",
            Self::Traded => "traded",
        }
    }
}

/// Append one event per order. Best effort by design: callers log failures,
/// a lost event degrades the status endpoint, never the competition.
pub async fn store(
    ex: impl PgExecutor<'_>,
    uids: impl IntoIterator<Item = IntentHash>,
    label: Label,
) -> Result<()> {
    let uids: Vec<Vec<u8>> = uids.into_iter().map(|uid| uid.0.to_vec()).collect();
    if uids.is_empty() {
        return Ok(());
    }
    sqlx::query(
        r#"
INSERT INTO solana.order_events (order_uid, timestamp, label)
SELECT uid, now(), $2::solana.OrderEventLabel FROM UNNEST($1::bytea[]) AS uid
        "#,
    )
    .bind(uids)
    .bind(label.as_str())
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

        store(&pool, [], Label::Ready).await.unwrap();
        store(
            &pool,
            [IntentHash([0x11; 32]), IntentHash([0x22; 32])],
            Label::Ready,
        )
        .await
        .unwrap();
        store(&pool, [IntentHash([0x11; 32])], Label::Executing)
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
