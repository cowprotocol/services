pub mod app_data;
pub mod auction;
pub mod auction_orders;
pub mod auction_participants;
pub mod auction_prices;
pub mod byte_array;
pub mod ethflow_orders;
pub mod events;
pub mod fee_policies;
pub mod jit_orders;
pub mod last_indexed_blocks;
pub mod onchain_broadcasted_orders;
pub mod onchain_invalidations;
pub mod order_events;
pub mod order_execution;
pub mod order_history;
pub mod orders;
pub mod quotes;
pub mod reference_scores;
pub mod settlement_executions;
pub mod settlement_observations;
pub mod settlement_scores;
pub mod settlements;
pub mod solver_competition;
pub mod surplus_capturing_jit_order_owners;
pub mod trades;

use {
    byte_array::ByteArray,
    sqlx::{Executor, PgPool},
};

// Design:
//
// Functions that execute multiple transactions should take `&mut PgTransaction`
// to indicate this and to ensure that the whole function succeeds or fails
// together. Functions that execute a single transaction should take `&mut
// PgConnection`. We usually call the parameter `ex` for `Executor` which is the
// trait whose methods we use to run queries.
// This scheme allows callers to decide whether they want to use the function as
// part of a bigger transaction or standalone. Note that PgTransaction
// implements Deref to PgConnection. Callers do need to take care of calling
// `commit` on the transaction.
//
// For tests a useful pattern is to start a transaction at the beginning of the
// test, use it for all queries and never commit it. When the uncommited
// transaction gets dropped it is rolled back. This allows postgres tests to run
// in parallel and makes clearing all tables at the beginning of a
// test obsolete.

pub type PgTransaction<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// The names of tables we use in the db.
pub const TABLES: &[&str] = &[
    "app_data",
    "auction_orders",
    "auctions",
    "competition_auctions",
    "ethflow_orders",
    "ethflow_refunds",
    "fee_policies",
    "interactions",
    "invalidations",
    "jit_orders",
    "last_indexed_blocks",
    "onchain_order_invalidations",
    "onchain_placed_orders",
    "order_execution",
    "order_quotes",
    "orders",
    "presignature_events",
    "proposed_jit_orders",
    "proposed_solutions",
    "quotes",
    "reference_scores",
    "settlement_executions",
    "settlement_observations",
    "settlement_scores",
    "settlements",
    "solver_competitions",
    "surplus_capturing_jit_order_owners",
    "trades",
];

/// The names of potentially big volume tables we use in the db.
pub const LARGE_TABLES: &[&str] = &[
    "auction_prices",
    "auction_participants",
    "order_events",
    "proposed_trade_executions",
];

pub fn all_tables() -> impl Iterator<Item = &'static str> {
    TABLES.iter().copied().chain(LARGE_TABLES.iter().copied())
}

/// Delete all data in the database. Only used by tests.
#[allow(non_snake_case)]
pub async fn clear_DANGER_(ex: &mut PgTransaction<'_>) -> sqlx::Result<()> {
    for table in all_tables() {
        ex.execute(format!("TRUNCATE {table};").as_str()).await?;
    }
    Ok(())
}

/// Like above but more ergonomic for some tests that use a pool.
#[allow(non_snake_case)]
pub async fn clear_DANGER(pool: &PgPool) -> sqlx::Result<()> {
    let mut transaction = pool.begin().await?;
    clear_DANGER_(&mut transaction).await?;
    transaction.commit().await
}

pub type Address = ByteArray<20>;
pub type AppId = ByteArray<32>;
pub type TransactionHash = ByteArray<32>;
pub type OrderUid = ByteArray<56>;

#[cfg(test)]
mod tests {
    use {
        super::*,
        sqlx::{Connection, PgConnection},
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_clear() {
        let mut con = PgConnection::connect("postgresql://").await.unwrap();
        let mut con = con.begin().await.unwrap();
        clear_DANGER_(&mut con).await.unwrap();
    }
}
