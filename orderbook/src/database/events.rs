use super::Database;
use crate::conversions::*;
use anyhow::{Context, Result};
use ethcontract::U256;
use futures::FutureExt;
use model::order::OrderUid;
use sqlx::{Connection, Executor, Postgres, Transaction};
use std::convert::TryInto;

#[derive(Debug)]
pub struct EventIndex {
    pub block_number: u64,
    pub log_index: u64,
}

#[derive(Debug)]
pub enum Event {
    Trade(Trade),
    Invalidation(Invalidation),
}

#[derive(Debug, Default)]
pub struct Trade {
    pub order_uid: OrderUid,
    pub sell_amount_including_fee: U256,
    pub buy_amount: U256,
    pub fee_amount: U256,
}

#[derive(Debug, Default)]
pub struct Invalidation {
    pub order_uid: OrderUid,
}

impl Database {
    pub async fn block_number_of_most_recent_event(&self) -> Result<u64> {
        const QUERY: &str = "\
            SELECT GREATEST( \
                (SELECT COALESCE(MAX(block_number), 0) FROM trades), \
                (SELECT COALESCE(MAX(block_number), 0) FROM invalidations));";
        let block_number: i64 = sqlx::query_scalar(QUERY)
            .fetch_one(&self.pool)
            .await
            .context("block_number_of_most_recent_trade failed")?;
        block_number.try_into().context("block number is negative")
    }

    // All insertions happen in one transaction.
    pub async fn insert_events(&self, events: Vec<(EventIndex, Event)>) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    insert_events(transaction, events.as_slice())
                        .await
                        .context("insert_events failed")
                }
                .boxed()
            })
            .await?;
        Ok(())
    }

    // The deletion and all insertions happen in one transaction.
    pub async fn replace_events(
        &self,
        delete_from_block_number: u64,
        events: Vec<(EventIndex, Event)>,
    ) -> Result<()> {
        let mut connection = self.pool.acquire().await?;
        connection
            .transaction(move |transaction| {
                async move {
                    delete_events(transaction, delete_from_block_number)
                        .await
                        .context("delete_events failed")?;
                    insert_events(transaction, events.as_slice())
                        .await
                        .context("insert_events failed")
                }
                .boxed()
            })
            .await?;
        Ok(())
    }
}

async fn delete_events(
    transaction: &mut Transaction<'_, Postgres>,
    delete_from_block_number: u64,
) -> Result<(), sqlx::Error> {
    const QUERY_INVALIDATION: &str = "DELETE FROM invalidations WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY_INVALIDATION).bind(delete_from_block_number as i64))
        .await?;

    const QUERY_TRADE: &str = "DELETE FROM trades WHERE block_number >= $1;";
    transaction
        .execute(sqlx::query(QUERY_TRADE).bind(delete_from_block_number as i64))
        .await?;

    Ok(())
}

async fn insert_events(
    transaction: &mut Transaction<'_, Postgres>,
    events: &[(EventIndex, Event)],
) -> Result<(), sqlx::Error> {
    // TODO: there might be a more efficient way to do this like execute_many or COPY but my
    // tests show that even if we sleep during the transaction it does not block other
    // connections from using the database, so it's not high priority.
    for (index, event) in events {
        match event {
            Event::Trade(event) => insert_trade(transaction, index, event).await?,
            Event::Invalidation(event) => insert_invalidation(transaction, index, event).await?,
        };
    }
    Ok(())
}

async fn insert_invalidation(
    transaction: &mut Transaction<'_, Postgres>,
    index: &EventIndex,
    event: &Invalidation,
) -> Result<(), sqlx::Error> {
    const QUERY: &str =
        "INSERT INTO invalidations (block_number, log_index, order_uid) VALUES ($1, $2, $3);";
    transaction
        .execute(
            sqlx::query(QUERY)
                .bind(index.block_number as i64)
                .bind(index.log_index as i64)
                .bind(event.order_uid.0.as_ref()),
        )
        .await?;
    Ok(())
}

async fn insert_trade(
    transaction: &mut Transaction<'_, Postgres>,
    index: &EventIndex,
    event: &Trade,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "INSERT INTO trades (block_number, log_index, order_uid, sell_amount, buy_amount, fee_amount) VALUES ($1, $2, $3, $4, $5, $6);";
    transaction
        .execute(
            sqlx::query(QUERY)
                .bind(index.block_number as i64)
                .bind(index.log_index as i64)
                .bind(event.order_uid.0.as_ref())
                .bind(u256_to_big_decimal(&event.sell_amount_including_fee))
                .bind(u256_to_big_decimal(&event.buy_amount))
                .bind(u256_to_big_decimal(&event.fee_amount)),
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn postgres_events() {
        let db = Database::new("postgresql://").unwrap();
        db.clear().await.unwrap();

        assert_eq!(db.block_number_of_most_recent_event().await.unwrap(), 0);

        db.insert_events(vec![(
            EventIndex {
                block_number: 1,
                log_index: 0,
            },
            Event::Invalidation(Invalidation::default()),
        )])
        .await
        .unwrap();
        assert_eq!(db.block_number_of_most_recent_event().await.unwrap(), 1);

        db.insert_events(vec![(
            EventIndex {
                block_number: 2,
                log_index: 0,
            },
            Event::Trade(Trade::default()),
        )])
        .await
        .unwrap();
        assert_eq!(db.block_number_of_most_recent_event().await.unwrap(), 2);

        db.replace_events(
            0,
            vec![(
                EventIndex {
                    block_number: 3,
                    log_index: 0,
                },
                Event::Invalidation(Invalidation::default()),
            )],
        )
        .await
        .unwrap();
        assert_eq!(db.block_number_of_most_recent_event().await.unwrap(), 3);

        db.replace_events(2, vec![]).await.unwrap();
        assert_eq!(db.block_number_of_most_recent_event().await.unwrap(), 0);
    }
}
