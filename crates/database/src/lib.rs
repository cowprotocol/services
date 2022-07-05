pub mod byte_array;

use byte_array::ByteArray;
use sqlx::PgExecutor;

// The names of all tables we use in the db.
pub const ALL_TABLES: &[&str] = &[
    "orders",
    "trades",
    "invalidations",
    "quotes",
    "settlements",
    "presignature_events",
    "order_quotes",
];

/// Delete all data in the database. Only used by tests.
#[allow(non_snake_case)]
pub async fn clear_DANGER(ex: impl PgExecutor<'_> + Copy) -> sqlx::Result<()> {
    for table in ALL_TABLES {
        ex.execute(format!("TRUNCATE {};", table).as_str()).await?;
    }
    Ok(())
}

pub type Address = ByteArray<20>;
pub type AppId = ByteArray<32>;
pub type TransactionHash = ByteArray<32>;
pub type OrderUid = ByteArray<56>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_clear() {
        let db = sqlx::PgPool::connect("postgresql://").await.unwrap();
        clear_DANGER(&db).await.unwrap();
    }
}
