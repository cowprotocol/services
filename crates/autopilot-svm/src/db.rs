//! Read access to the `solana.*` tables the indexer writes.
//!
//! Runtime SQL, no compile-time macros: the schema lives on a separate
//! migration branch and is applied out of band, so queries are checked when
//! the ignored DB tests run against it, not at build time.

use {
    anyhow::{Context, Result},
    database::byte_array::ByteArray,
    sqlx::PgExecutor,
};

/// A row of `solana.settlements`, the indexer's record of one settlement
/// transaction. `solution_uid` is filled in later by the indexer, so it is
/// nullable.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Settlement {
    pub slot: i64,
    pub tx_signature: ByteArray<64>,
    pub solver: ByteArray<32>,
    pub auction_id: i64,
    pub solution_uid: Option<i64>,
    pub commitment: String,
}

/// Latest slot the indexer has processed at `confirmed`. `None` before the
/// indexer's first write.
pub async fn slot_watermark(ex: impl PgExecutor<'_>) -> Result<Option<i64>> {
    const QUERY: &str = r#"SELECT last_indexed_slot FROM solana.indexer_state WHERE id = 0"#;
    sqlx::query_scalar(QUERY)
        .fetch_optional(ex)
        .await
        .context("read solana.indexer_state watermark")
}

/// Settlements the indexer recorded for an auction. More than one row when the
/// auction had several winners.
pub async fn settlements_by_auction(
    ex: impl PgExecutor<'_>,
    auction_id: i64,
) -> Result<Vec<Settlement>> {
    const QUERY: &str = r#"
SELECT slot, tx_signature, solver, auction_id, solution_uid, commitment
FROM solana.settlements
WHERE auction_id = $1
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
        super::{settlements_by_auction, slot_watermark},
        database::byte_array::ByteArray,
        sqlx::PgPool,
    };

    // Inserts a row into each read table and reads it back. Runs inside a
    // transaction that rolls back, so it leaves no residue. Needs the
    // `solana.*` schema applied out of band (the migration branch).
    #[tokio::test]
    #[ignore = "needs the solana.* schema, run manually against the migration branch"]
    async fn reads_round_trip() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let mut tx = pool.begin().await.unwrap();

        sqlx::query(
            r#"
INSERT INTO solana.indexer_state (id, last_indexed_slot)
VALUES (0, 42)
ON CONFLICT (id) DO UPDATE SET last_indexed_slot = EXCLUDED.last_indexed_slot
            "#,
        )
        .execute(&mut *tx)
        .await
        .unwrap();
        assert_eq!(slot_watermark(&mut *tx).await.unwrap(), Some(42));

        sqlx::query(
            r#"
INSERT INTO solana.settlements (slot, tx_signature, solver, auction_id, solution_uid, commitment)
VALUES (7, $1, $2, 123, NULL, 'finalized')
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
        assert_eq!(settlements[0].commitment, "finalized");
    }
}
