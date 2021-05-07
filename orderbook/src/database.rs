mod events;
mod fees;
mod orders;
mod trades;

use anyhow::Result;
use sqlx::{Executor, PgPool, Row};
use std::collections::HashMap;

pub use events::*;
pub use orders::OrderFilter;
pub use trades::TradeFilter;

// TODO: There is remaining optimization potential by implementing sqlx encoding and decoding for
// U256 directly instead of going through BigDecimal. This is not very important as this is fast
// enough anyway.

// The names of all tables we use in the db.
const ALL_TABLES: [&str; 5] = [
    "orders",
    "trades",
    "invalidations",
    "min_fee_measurements",
    "settlements",
];

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

#[derive(Debug)]
pub enum InsertionError {
    DuplicatedRecord,
    DbError(sqlx::Error),
}

impl From<sqlx::Error> for InsertionError {
    fn from(err: sqlx::Error) -> Self {
        Self::DbError(err)
    }
}

// The implementation is split up into several modules which contain more public methods.

impl Database {
    pub fn new(uri: &str) -> Result<Self> {
        Ok(Self {
            pool: PgPool::connect_lazy(uri)?,
        })
    }

    /// Delete all data in the database. Only used by tests.
    pub async fn clear(&self) -> Result<()> {
        for table in ALL_TABLES.iter() {
            self.pool
                .execute(format!("TRUNCATE {};", table).as_str())
                .await?;
        }
        Ok(())
    }

    async fn count_rows_in_table(&self, table: &str) -> Result<i64> {
        let query = format!("SELECT COUNT(*) FROM {};", table);
        let row = self.pool.fetch_one(query.as_str()).await?;
        row.try_get(0).map_err(Into::into)
    }

    pub async fn count_rows_in_tables(&self) -> Result<HashMap<&'static str, i64>> {
        let mut result = HashMap::new();
        for &table in ALL_TABLES.iter() {
            result.insert(table, self.count_rows_in_table(table).await?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_count_rows_in_tables_works() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        let counts = db.count_rows_in_tables().await.unwrap();
        assert_eq!(counts.len(), 5);
        assert!(counts.iter().all(|(_, count)| *count == 0));

        db.insert_order(&Default::default()).await.unwrap();
        let counts = db.count_rows_in_tables().await.unwrap();
        assert_eq!(counts.get("orders"), Some(&1));
    }
}
