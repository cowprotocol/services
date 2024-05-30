use {
    crate::{events::EventIndex, PgTransaction, TransactionHash},
    sqlx::{Executor, PgConnection},
};

pub async fn get_hash_by_event(
    ex: &mut PgConnection,
    event: &EventIndex,
) -> Result<TransactionHash, sqlx::Error> {
    const QUERY: &str = r#"
SELECT tx_hash
FROM settlements
WHERE
    block_number = $1 AND
    log_index = $2
    "#;
    sqlx::query_scalar::<_, TransactionHash>(QUERY)
        .bind(event.block_number)
        .bind(event.log_index)
        .fetch_one(ex)
        .await
}

pub async fn get_hash_by_auction_id(
    ex: &mut PgConnection,
    auction_id: i64,
) -> Result<Option<TransactionHash>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT tx_hash
FROM settlements
WHERE
    auction_id = $1
    "#;
    sqlx::query_scalar::<_, TransactionHash>(QUERY)
        .bind(auction_id)
        .fetch_optional(ex)
        .await
}

#[derive(Debug, sqlx::FromRow)]
pub struct SettlementEvent {
    pub block_number: i64,
    pub log_index: i64,
    pub tx_hash: TransactionHash,
}

pub async fn get_settlement_without_auction(
    ex: &mut PgConnection,
) -> Result<Option<SettlementEvent>, sqlx::Error> {
    Ok(get_settlements_without_auction(ex, 1).await?.pop())
}

pub async fn get_settlements_without_auction(
    ex: &mut PgConnection,
    limit: i64,
) -> Result<Vec<SettlementEvent>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number, log_index, tx_hash
FROM settlements
WHERE auction_id IS NULL
ORDER BY block_number ASC
LIMIT $1
    "#;
    sqlx::query_as(QUERY).bind(limit).fetch_all(ex).await
}

pub async fn already_processed(
    ex: &mut PgConnection,
    auction_id: i64,
) -> Result<bool, sqlx::Error> {
    const QUERY: &str = r#"SELECT COUNT(*) FROM settlements WHERE auction_id = $1;"#;
    let count: i64 = sqlx::query_scalar(QUERY)
        .bind(auction_id)
        .fetch_one(ex)
        .await?;
    Ok(count >= 1)
}

pub async fn update_settlement_auction(
    ex: &mut PgConnection,
    block_number: i64,
    log_index: i64,
    auction_id: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlements
SET auction_id = $1
WHERE block_number = $2 AND log_index = $3
    ;"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(block_number)
        .bind(log_index)
        .execute(ex)
        .await
        .map(|_| ())
}

pub async fn delete(
    ex: &mut PgTransaction<'_>,
    delete_from_block_number: u64,
) -> Result<(), sqlx::Error> {
    const QUERY_OBSERVATIONS: &str =
        "DELETE FROM settlement_observations WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_OBSERVATIONS).bind(delete_from_block_number as i64))
        .await?;

    const QUERY_ORDER_EXECUTIONS: &str = "DELETE FROM order_execution WHERE block_number >= $1;";
    ex.execute(sqlx::query(QUERY_ORDER_EXECUTIONS).bind(delete_from_block_number as i64))
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            events::{Event, EventIndex, Settlement},
        },
        sqlx::Connection,
    };

    async fn all_settlement_tx_hashes(
        ex: &mut PgConnection,
    ) -> Result<Vec<TransactionHash>, sqlx::Error> {
        const QUERY: &str = "SELECT tx_hash FROM settlements";
        sqlx::query_scalar::<_, TransactionHash>(QUERY)
            .fetch_all(ex)
            .await
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_gets_event() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        crate::events::append(
            &mut db,
            &[
                (
                    EventIndex {
                        block_number: 0,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([0u8; 32]),
                    }),
                ),
                (
                    EventIndex {
                        block_number: 1,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([1u8; 32]),
                    }),
                ),
                (
                    EventIndex {
                        block_number: 2,
                        log_index: 0,
                    },
                    Event::Settlement(Settlement {
                        solver: Default::default(),
                        transaction_hash: ByteArray([2u8; 32]),
                    }),
                ),
            ],
        )
        .await
        .unwrap();

        let results = all_settlement_tx_hashes(&mut db).await.unwrap();
        assert_eq!(
            results,
            &[
                ByteArray([0u8; 32]),
                ByteArray([1u8; 32]),
                ByteArray([2u8; 32])
            ]
        );
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_settlement_auction() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event = Default::default();
        crate::events::insert_settlement(&mut db, &event, &Default::default())
            .await
            .unwrap();

        let settlement = get_settlement_without_auction(&mut db)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(settlement.block_number, event.block_number);
        assert_eq!(settlement.log_index, event.log_index);

        update_settlement_auction(&mut db, event.block_number, event.log_index, 1)
            .await
            .unwrap();

        let settlement = get_settlement_without_auction(&mut db).await.unwrap();

        assert!(settlement.is_none());
    }
}
