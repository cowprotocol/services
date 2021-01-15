mod orders;
mod trades;

use anyhow::Result;
use sqlx::PgPool;

pub use orders::OrderFilter;
pub use trades::Trade;

// TODO: There is remaining optimization potential by implementing sqlx encoding and decoding for
// U256 directly instead of going through BigDecimal. This is not very important as this is fast
// enough anyway.

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

// The implementation is split up into the orders and trades modules which contain more public
// methods.

impl Database {
    pub fn new(uri: &str) -> Result<Self> {
        Ok(Self {
            pool: PgPool::connect_lazy(uri)?,
        })
    }

    #[cfg(test)]
    /// Delete all data in the database.
    async fn clear(&self) -> Result<()> {
        use sqlx::Executor;
        self.pool.execute(sqlx::query("TRUNCATE orders;")).await?;
        self.pool.execute(sqlx::query("TRUNCATE trades;")).await?;
        Ok(())
    }
}
