pub mod app_data;
pub mod auction;
pub mod auction_prices;
pub mod byte_array;
pub mod cow_amms;
pub mod ethflow_orders;
pub mod events;
pub mod fee_policies;
pub mod jit_orders;
pub mod last_indexed_blocks;
pub mod leader_pg_lock;
pub mod onchain_broadcasted_orders;
pub mod onchain_invalidations;
pub mod order_events;
pub mod order_execution;
pub mod order_history;
pub mod orders;
pub mod quotes;
pub mod reference_scores;
pub mod settlement_executions;
pub mod settlements;
pub mod solver_competition;
pub mod solver_competition_v2;
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
    "auctions",
    "cow_amms",
    "ethflow_orders",
    "ethflow_refunds",
    "interactions",
    "invalidations",
    "jit_orders",
    "last_indexed_blocks",
    "onchain_order_invalidations",
    "onchain_placed_orders",
    "presignature_events",
    "proposed_jit_orders",
    "quotes",
    "reference_scores",
    "settlement_executions",
    "settlements",
    "solver_competitions",
    "trades",
];

/// The names of potentially big volume tables we use in the db.
pub const LARGE_TABLES: &[&str] = &[
    "auction_prices",
    "competition_auctions",
    "fee_policies",
    "orders",
    "order_events",
    "order_execution",
    "order_quotes",
    "proposed_solutions",
    "proposed_trade_executions",
];

pub fn all_tables() -> impl Iterator<Item = &'static str> {
    TABLES.iter().copied().chain(LARGE_TABLES.iter().copied())
}

/// Delete all data in the database. Only used by tests.
///
/// Truncates all tables in a single statement so Postgres accepts foreign-key
/// cycles between listed tables. Individual per-table `TRUNCATE`s error out
/// when any other listed table references the one being truncated.
#[expect(non_snake_case)]
pub async fn clear_DANGER_(ex: &mut PgTransaction<'_>) -> sqlx::Result<()> {
    let tables = all_tables().collect::<Vec<_>>().join(", ");
    ex.execute(format!("TRUNCATE {tables};").as_str()).await?;
    Ok(())
}

/// Like above but more ergonomic for some tests that use a pool.
#[expect(non_snake_case)]
pub async fn clear_DANGER(pool: &PgPool) -> sqlx::Result<()> {
    let mut transaction = pool.begin().await?;
    clear_DANGER_(&mut transaction).await?;
    transaction.commit().await
}

pub type Address = ByteArray<20>;
pub type AppId = ByteArray<32>;
pub type TransactionHash = ByteArray<32>;
pub type OrderUid = ByteArray<56>;

/// Returns references to the elements of `items`, keeping only the **last**
/// occurrence of each key while preserving the original relative order.
///
/// A batched `INSERT ... ON CONFLICT (...) DO UPDATE` statement errors with
/// "ON CONFLICT DO UPDATE command cannot affect row a second time" if the same
/// conflict key appears more than once in the same statement. The single-row
/// loops these batches replace were immune to this (a later row simply
/// overwrote an earlier one), so we reproduce that "last write wins" semantics
/// by de-duplicating before building the batch.
pub(crate) fn dedup_keep_last<T, K, F>(items: &[T], mut key: F) -> Vec<&T>
where
    F: FnMut(&T) -> K,
    K: std::hash::Hash + Eq,
{
    let mut seen = std::collections::HashSet::with_capacity(items.len());
    let mut keep = vec![false; items.len()];
    // Walk backwards so the first key we see for a given value is its last
    // occurrence.
    for (i, item) in items.iter().enumerate().rev() {
        if seen.insert(key(item)) {
            keep[i] = true;
        }
    }
    items
        .iter()
        .zip(keep)
        .filter_map(|(item, keep)| keep.then_some(item))
        .collect()
}

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
