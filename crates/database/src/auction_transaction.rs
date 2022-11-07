use crate::{auction::AuctionId, Address, TransactionHash};
use sqlx::{postgres::PgQueryResult, PgConnection};

/// "upsert" because we might have previously unsuccessfully attempted to settle an auction with the
/// same address-nonce.
pub async fn upsert_auction_transaction(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    tx_from: &Address,
    tx_nonce: i64,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO auction_transaction (auction_id, tx_from, tx_nonce)
VALUES ($1, $2, $3)
ON CONFLICT (tx_from, tx_nonce) DO UPDATE
SET auction_id = EXCLUDED.auction_id
    ;"#;
    sqlx::query(QUERY)
        .bind(auction_id)
        .bind(tx_from)
        .bind(tx_nonce)
        .execute(ex)
        .await?;
    Ok(())
}

pub async fn insert_settlement_tx_info(
    ex: &mut PgConnection,
    block_number: i64,
    log_index: i64,
    tx_from: &Address,
    tx_nonce: i64,
) -> Result<PgQueryResult, sqlx::Error> {
    const QUERY: &str = r#"
UPDATE settlements
SET tx_from = $1, tx_nonce = $2
WHERE block_number = $3 AND log_index = $4
    ;"#;
    sqlx::query(QUERY)
        .bind(tx_from)
        .bind(tx_nonce)
        .bind(block_number)
        .bind(log_index)
        .execute(ex)
        .await
}

#[derive(Debug, sqlx::FromRow)]
pub struct SettlementEvent {
    pub block_number: i64,
    pub log_index: i64,
    pub tx_hash: TransactionHash,
}

pub async fn get_settlement_event_without_tx_info(
    ex: &mut PgConnection,
    max_block_number: i64,
) -> Result<Option<SettlementEvent>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT block_number, log_index, tx_hash
FROM settlements
WHERE tx_from IS NULL AND block_number <= $1
LIMIT 1
    "#;
    sqlx::query_as(QUERY)
        .bind(max_block_number)
        .fetch_optional(ex)
        .await
}

pub async fn get_auction_id_from_tx_hash(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Option<i64>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT id
FROM solver_competitions
WHERE tx_hash = $1
LIMIT 1
    "#;
    sqlx::query_scalar(QUERY)
        .bind(tx_hash)
        .fetch_optional(ex)
        .await
}

#[cfg(test)]
mod tests {
    use sqlx::Connection;

    use crate::{
        byte_array::ByteArray,
        events::{Event, EventIndex, Settlement},
    };

    use super::*;

    fn is_duplicate_auction_id_error(err: &sqlx::Error) -> bool {
        match err {
            sqlx::Error::Database(err) => err.constraint() == Some("auction_transaction_pkey"),
            _ => false,
        }
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_double_insert_error() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        upsert_auction_transaction(&mut db, 0, &Default::default(), 0)
            .await
            .unwrap();

        // Doesn't error because whole row is the same.
        upsert_auction_transaction(&mut db, 0, &Default::default(), 0)
            .await
            .unwrap();

        // Errors because of primary key violation.
        let err = upsert_auction_transaction(&mut db, 0, &Default::default(), 1)
            .await
            .unwrap_err();
        dbg!(&err);
        assert!(is_duplicate_auction_id_error(&err));
    }

    #[tokio::test]
    #[ignore]
    async fn upsert_auction_transaction_() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        upsert_auction_transaction(&mut db, 0, &Default::default(), 0)
            .await
            .unwrap();

        // same account-nonce other auction_id
        upsert_auction_transaction(&mut db, 1, &Default::default(), 0)
            .await
            .unwrap();

        let auction_id: i64 = sqlx::query_scalar("SELECT auction_id FROM auction_transaction")
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(auction_id, 1);
    }

    #[tokio::test]
    #[ignore]
    async fn insert_settlement_tx_info_() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let index = EventIndex::default();
        let event = Event::Settlement(Settlement {
            solver: Default::default(),
            transaction_hash: Default::default(),
        });
        crate::events::append(&mut db, &[(index, event)])
            .await
            .unwrap();

        let auction_id: Option<i64> = sqlx::query_scalar("SELECT tx_nonce FROM settlements")
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(auction_id, None);

        insert_settlement_tx_info(&mut db, 0, 0, &Default::default(), 1)
            .await
            .unwrap();

        let auction_id: Option<i64> = sqlx::query_scalar("SELECT tx_nonce FROM settlements")
            .fetch_one(&mut db)
            .await
            .unwrap();
        assert_eq!(auction_id, Some(1));
    }

    #[tokio::test]
    #[ignore]
    async fn get_settlement_event_without_tx_info_() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let event = get_settlement_event_without_tx_info(&mut db, 10)
            .await
            .unwrap();
        assert!(event.is_none());

        // event at block 0
        let index = EventIndex::default();
        let event = Event::Settlement(Settlement {
            solver: Default::default(),
            transaction_hash: Default::default(),
        });
        crate::events::append(&mut db, &[(index, event)])
            .await
            .unwrap();

        // is found
        let event = get_settlement_event_without_tx_info(&mut db, 10)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.block_number, 0);

        // gets tx info
        insert_settlement_tx_info(&mut db, 0, 0, &Default::default(), 1)
            .await
            .unwrap();

        // no longer found
        let event = get_settlement_event_without_tx_info(&mut db, 10)
            .await
            .unwrap();
        assert!(event.is_none());

        // event at 11
        let index = EventIndex {
            block_number: 11,
            log_index: 0,
        };
        let event = Event::Settlement(Settlement {
            solver: Default::default(),
            transaction_hash: Default::default(),
        });
        crate::events::append(&mut db, &[(index, event)])
            .await
            .unwrap();

        // not found because block number too large
        let event = get_settlement_event_without_tx_info(&mut db, 10)
            .await
            .unwrap();
        assert!(event.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn get_auction_id_from_tx_hash_() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let hash = ByteArray::<32>::default();

        let id = get_auction_id_from_tx_hash(&mut db, &hash).await.unwrap();
        assert!(id.is_none());

        crate::solver_competition::save(&mut db, 1, &Default::default(), Some(&hash))
            .await
            .unwrap();

        let id = get_auction_id_from_tx_hash(&mut db, &hash).await.unwrap();
        assert_eq!(id, Some(1));
    }
}
