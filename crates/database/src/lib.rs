pub mod byte_array;
pub mod events;
pub mod orders;
pub mod quotes;
pub mod solver_competition;
pub mod trades;

use byte_array::ByteArray;
use sqlx::{Executor, PgPool};

// Design:
//
// Functions that execute multiple transactions should take `&mut PgTransaction` to indicate this
// and to ensure that the whole function succeeds or fails together.
// Functions that execute a single transaction should take `&mut PgConnection`.
// We usually call the parameter `ex` for `Executor` which is the trait whose methods we use to run
// queries.
// This scheme allows callers to decide whether they want to use the function as part of a bigger
// transaction or standalone. Note that PgTransaction implements Deref to PgConnection.
// Callers do need to take care of calling `commit` on the transaction.
//
// For tests a useful pattern is to start a transaction at the beginning of the test, use it for all
// queries and never commit it. When the uncommited transaction gets dropped it is rolled back. This
// allows postgres tests to run in parallel and makes clearing all tables at the beginning of a
// test obsolete.

pub type PgTransaction<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

/// The names of all tables we use in the db.
pub const ALL_TABLES: &[&str] = &[
    "orders",
    "trades",
    "invalidations",
    "quotes",
    "settlements",
    "presignature_events",
    "order_quotes",
    "solver_competitions",
];

/// Delete all data in the database. Only used by tests.
#[allow(non_snake_case)]
pub async fn clear_DANGER_(ex: &mut PgTransaction<'_>) -> sqlx::Result<()> {
    for table in ALL_TABLES {
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
    use super::*;
    use sqlx::{Connection, PgConnection};

    #[tokio::test]
    #[ignore]
    async fn postgres_clear() {
        let mut con = PgConnection::connect("postgresql://").await.unwrap();
        let mut con = con.begin().await.unwrap();
        clear_DANGER_(&mut con).await.unwrap();
    }
}
