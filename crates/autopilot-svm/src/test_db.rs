//! Database helpers for the crate's `solana_db_` tests.

use sqlx::PgPool;

pub(crate) async fn pool() -> PgPool {
    PgPool::connect("postgresql://").await.unwrap()
}

/// Empty every `solana.*` table a test can touch. Tests wipe at the start so
/// a failure leaves its state behind for inspection.
pub(crate) async fn wipe(pool: &PgPool) {
    sqlx::query(
        "TRUNCATE solana.trades, solana.settlements, solana.settlement_executions, \
         solana.order_pda, solana.orders, solana.indexer_state",
    )
    .execute(pool)
    .await
    .unwrap();
}
