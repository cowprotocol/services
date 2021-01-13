mod orders;
mod trades;

use crate::u256_conversions::*;
use anyhow::{Context, Result};
use futures::FutureExt;
use model::order::OrderUid;
use primitive_types::U256;
use sqlx::{Connection, Executor, PgPool, Postgres, Transaction};
use std::convert::TryInto;

// TODO: There is remaining optimization potential by implementing sqlx encoding and decoding for
// U256 directly instead of going through BigDecimal. This is not very important as this is fast
// enough anyway.

#[derive(Debug, Default)]
pub struct Trade {
    pub block_number: u64,
    pub log_index: u64,
    pub order_uid: OrderUid,
    pub sell_amount: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
}

// The pool uses an Arc internally.
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

// The implementation is split up into the orders and trades modules.

impl Database {
    pub fn new(uri: &str) -> Result<Self> {
        Ok(Self {
            pool: PgPool::connect_lazy(uri)?,
        })
    }

    pub async fn block_number_of_most_recent_trade(&self) -> Result<Option<u64>> {
        const QUERY: &str = "SELECT MAX(block_number) FROM trades;";
        let block_number: Option<i64> = sqlx::query_scalar(QUERY)
            .fetch_one(&self.pool)
            .await
            .context("block_number_of_most_recent_trade failed")?;
        block_number
            .map(|block_number| block_number.try_into().context("block number is negative"))
            .transpose()
    }

    // All insertions happen in one transaction.
    pub async fn insert_trades(&self, trades: Vec<Trade>) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    insert_trades(transaction, trades.as_slice())
                        .await
                        .context("insert_trades failed")
                }
                .boxed()
            })
            .await?;
        Ok(())
    }

    // The deletion and all insertions happen in one transaction.
    pub async fn replace_trades(
        &self,
        delete_from_block_number: u64,
        trades: Vec<Trade>,
    ) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    delete_trades(transaction, delete_from_block_number)
                        .await
                        .context("delete_trades failed")?;
                    insert_trades(transaction, trades.as_slice())
                        .await
                        .context("insert_trades failed")
                }
                .boxed()
            })
            .await?;
        Ok(())
    }

    #[cfg(test)]
    /// Delete all data in the database.
    async fn clear(&self) -> Result<()> {
        self.pool.execute(sqlx::query("TRUNCATE orders;")).await?;
        self.pool.execute(sqlx::query("TRUNCATE trades;")).await?;
        Ok(())
    }
}

async fn delete_trades(
    transaction: &mut Transaction<'_, Postgres>,
    delete_from_block_number: u64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "DELETE FROM trades WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY).bind(delete_from_block_number as i64))
        .await?;
    Ok(())
}

async fn insert_trades(
    transaction: &mut Transaction<'_, Postgres>,
    trades: &[Trade],
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "INSERT INTO trades (block_number, log_index, order_uid, sell_amount, buy_amount, fee_amount) VALUES ($1, $2, $3, $4, $5, $6);";
    for trade in trades {
        // TODO: there might be a more efficient way to do this like execute_many or COPY but my
        // tests show that even if we sleep during the transaction it does not block other
        // connections from using the database, so it's not high priority.
        transaction
            .execute(
                sqlx::query(QUERY)
                    .bind(trade.block_number as i64)
                    .bind(trade.log_index as i64)
                    .bind(trade.order_uid.0.as_ref())
                    .bind(u256_to_big_decimal(&trade.sell_amount))
                    .bind(u256_to_big_decimal(&trade.buy_amount))
                    .bind(u256_to_big_decimal(&trade.fee_amount)),
            )
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn trade(block_number: u64, log_index: u64) -> Trade {
        Trade {
            block_number,
            log_index,
            ..Default::default()
        }
    }

    // Needs a local postgres instance running as we have in CI.
    #[tokio::test]
    #[ignore]
    async fn postgres_trades() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();
        assert_eq!(db.block_number_of_most_recent_trade().await.unwrap(), None);
        db.insert_trades(vec![trade(0, 0), trade(0, 1)])
            .await
            .unwrap();
        assert_eq!(
            db.block_number_of_most_recent_trade().await.unwrap(),
            Some(0)
        );
        db.replace_trades(1, vec![]).await.unwrap();
        assert_eq!(
            db.block_number_of_most_recent_trade().await.unwrap(),
            Some(0)
        );
        db.replace_trades(0, vec![trade(1, 0)]).await.unwrap();
        assert_eq!(
            db.block_number_of_most_recent_trade().await.unwrap(),
            Some(1)
        );
        db.replace_trades(0, vec![]).await.unwrap();
        assert_eq!(db.block_number_of_most_recent_trade().await.unwrap(), None);
    }
}
