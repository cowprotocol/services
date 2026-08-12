//! Database access for the Solana autopilot.

use {
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    sqlx::PgExecutor,
};

/// A row of `solana.settlements`, the indexer's record of one settlement
/// transaction. The transaction carries no solution uid, the indexer
/// attributes it from the recorded competition, so `solution_uid` is `None`
/// for settlements it cannot match. A settlement is finalized once its slot
/// is at or below the watermark's `finalized_slot`.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Settlement {
    pub slot: i64,
    pub tx_signature: ByteArray<64>,
    pub solver: ByteArray<32>,
    pub auction_id: i64,
    pub solution_uid: Option<i64>,
}

/// Latest slot the indexer fully processed. `None` before the indexer's first
/// write. `solana.indexer_state` is a single-row table.
pub async fn last_indexed_slot(ex: impl PgExecutor<'_>) -> Result<Option<i64>> {
    const QUERY: &str = r#"SELECT slot FROM solana.indexer_state"#;
    sqlx::query_scalar(QUERY)
        .fetch_optional(ex)
        .await
        .context("read solana.indexer_state slot")
}

/// Settlements the indexer recorded for an auction. More than one row when the
/// auction had several winners.
pub async fn settlements_by_auction(
    ex: impl PgExecutor<'_>,
    auction_id: i64,
) -> Result<Vec<Settlement>> {
    const QUERY: &str = r#"
SELECT slot, tx_signature, solver, auction_id, solution_uid
FROM solana.settlements
WHERE auction_id = $1
ORDER BY slot, tx_signature
    "#;
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .fetch_all(ex)
        .await
        .context("read solana.settlements by auction")
}

#[cfg(test)]
mod tests {
    use {
        super::{last_indexed_slot, settlements_by_auction},
        database::byte_array::ByteArray,
        sqlx::PgPool,
    };

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn postgres_last_indexed_slot_roundtrip() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let mut tx = pool.begin().await.unwrap();

        sqlx::query(r#"DELETE FROM solana.indexer_state"#)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(last_indexed_slot(&mut *tx).await.unwrap(), None);

        sqlx::query(r#"INSERT INTO solana.indexer_state (slot, finalized_slot) VALUES (42, 0)"#)
            .execute(&mut *tx)
            .await
            .unwrap();
        assert_eq!(last_indexed_slot(&mut *tx).await.unwrap(), Some(42));
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn postgres_settlements_by_auction_roundtrip() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let mut tx = pool.begin().await.unwrap();

        sqlx::query(r#"DELETE FROM solana.settlements"#)
            .execute(&mut *tx)
            .await
            .unwrap();
        sqlx::query(
            r#"
INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, auction_id, solution_uid)
VALUES (7, $1, 0, $2, 123, NULL)
            "#,
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray([10u8; 32]))
        .execute(&mut *tx)
        .await
        .unwrap();
        let settlements = settlements_by_auction(&mut *tx, 123).await.unwrap();
        assert_eq!(settlements.len(), 1);
        assert_eq!(settlements[0].auction_id, 123);
        assert_eq!(settlements[0].solution_uid, None);
        assert_eq!(settlements[0].slot, 7);
    }
}
