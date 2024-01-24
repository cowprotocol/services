use {
    crate::{events::EventIndex, TransactionHash},
    sqlx::PgConnection,
    std::ops::Range,
};

pub async fn recent_settlement_tx_hashes(
    ex: &mut PgConnection,
    block_range: Range<i64>,
) -> Result<Vec<TransactionHash>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT tx_hash
FROM settlements
WHERE
    block_number >= $1 AND
    block_number < $2
    "#;
    sqlx::query_scalar::<_, TransactionHash>(QUERY)
        .bind(block_range.start)
        .bind(block_range.end)
        .fetch_all(ex)
        .await
}

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

#[derive(Debug, sqlx::FromRow)]
pub struct SettlementEvent {
    pub block_number: i64,
    pub log_index: i64,
    pub tx_hash: TransactionHash,
}

pub async fn get_settlement_without_auction(
    ex: &mut PgConnection,
    max_block_number: i64,
) -> Result<Option<SettlementEvent>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number, log_index, tx_hash
FROM settlements
WHERE auction_id IS NULL
AND block_number <= $1
ORDER BY block_number ASC
LIMIT 1
    "#;
    sqlx::query_as(QUERY)
        .bind(max_block_number)
        .fetch_optional(ex)
        .await
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

        let results = recent_settlement_tx_hashes(&mut db, 0..1).await.unwrap();
        assert_eq!(results, &[ByteArray([0u8; 32])]);

        let results = recent_settlement_tx_hashes(&mut db, 1..5).await.unwrap();
        assert_eq!(results, &[ByteArray([1u8; 32]), ByteArray([2u8; 32])]);

        let results = recent_settlement_tx_hashes(&mut db, 2..5).await.unwrap();
        assert_eq!(results, &[ByteArray([2u8; 32])]);

        let results = recent_settlement_tx_hashes(&mut db, 3..5).await.unwrap();
        assert_eq!(results, &[]);
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

        let settlement = get_settlement_without_auction(&mut db, 0)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(settlement.block_number, event.block_number);
        assert_eq!(settlement.log_index, event.log_index);

        update_settlement_auction(&mut db, event.block_number, event.log_index, 1)
            .await
            .unwrap();

        let settlement = get_settlement_without_auction(&mut db, 0).await.unwrap();

        assert!(settlement.is_none());
    }
}
