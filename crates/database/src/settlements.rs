use {
    crate::{events::EventIndex, TransactionHash},
    sqlx::{postgres::PgQueryResult, PgConnection},
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

#[derive(Debug, Clone, PartialEq, sqlx::Type)]
#[sqlx(type_name = "AuctionKind", rename_all = "lowercase")]
pub enum AuctionKind {
    Valid,
    Invalid,
}

pub async fn get_settlement_without_auction(
    ex: &mut PgConnection,
) -> Result<Option<SettlementEvent>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number, log_index, tx_hash
FROM settlements
WHERE auction_kind = 'unprocessed'
LIMIT 1
    "#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

pub async fn update_settlement_auction(
    ex: &mut PgConnection,
    block_number: i64,
    log_index: i64,
    auction_id: Option<i64>,
    auction_kind: AuctionKind,
) -> Result<PgQueryResult, sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlements
SET auction_kind = $1, auction_id = $2
WHERE block_number = $3 AND log_index = $4
    ;"#;
    sqlx::query(QUERY)
        .bind(auction_kind)
        .bind(auction_id)
        .bind(block_number)
        .bind(log_index)
        .execute(ex)
        .await
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
}
