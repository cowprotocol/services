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
pub mod settlement_observations;
pub mod settlement_scores;
pub mod settlements;
pub mod solver_competition;
pub mod surplus_capturing_jit_order_owners;
pub mod trades;

use {
    byte_array::ByteArray,
    sqlx::{Executor, PgConnection, PgPool, QueryBuilder},
    tokio::sync::OnceCell,
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
static TABLES: OnceCell<&'static [&'static str]> = OnceCell::const_new();

/// The names of potentially big volume tables we use in the db.
pub const LARGE_TABLES: &[&str] = &["auction_prices", "order_events"];

pub async fn get_table_names(ex: &mut PgConnection) -> sqlx::Result<&'static [&'static str]> {
    TABLES
        .get_or_try_init(|| async {
            #[derive(sqlx::FromRow, Debug)]
            struct TableName(String);

            let mut query_builder = QueryBuilder::new(
                "SELECT tablename FROM pg_tables WHERE schemaname = 'public' AND tablename NOT \
                 LIKE '%flyway%' AND tablename NOT IN (",
            );

            query_builder.push_values(LARGE_TABLES, |mut builder, table| {
                builder.push_bind(table);
            });
            query_builder.push(")");

            let mut table_names: Vec<&'static str> = query_builder
                .build_query_as::<TableName>()
                .fetch_all(ex)
                .await?
                .into_iter()
                .map(|TableName(name)| {
                    let leaked: &'static mut str = Box::leak(name.into_boxed_str());
                    leaked as &'static str
                })
                .collect();

            table_names.extend(LARGE_TABLES.iter().copied());

            let leaked_slice: &'static mut [&'static str] =
                Box::leak(table_names.into_boxed_slice());
            Ok(&*leaked_slice)
        })
        .await
        .copied()
}

pub async fn all_tables(ex: &mut PgConnection) -> sqlx::Result<impl Iterator<Item = &'static str>> {
    Ok(get_table_names(ex)
        .await?
        .iter()
        .copied()
        .chain(LARGE_TABLES.iter().copied()))
}

/// Delete all data in the database. Only used by tests.
#[allow(non_snake_case)]
pub async fn clear_DANGER_(ex: &mut PgTransaction<'_>) -> sqlx::Result<()> {
    for table in all_tables(ex).await? {
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
