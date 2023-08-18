use {
    crate::{auction::AuctionId, Address, TransactionHash},
    sqlx::{postgres::PgQueryResult, PgConnection},
};

/// "upsert" because we might have previously unsuccessfully attempted to settle
/// an auction with the same address-nonce.
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

/// Inserts a row **iff** we don't have an entry for the given `auction_id` yet.
/// This is useful to associate a settlement transaction coming from a colocated
/// driver with an auction.
/// In that case anybody could claim to settle the given auction but we only
/// ever want to store the first claim.
pub async fn try_insert_auction_transaction(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    tx_from: &Address,
    tx_nonce: i64,
) -> Result<bool, sqlx::Error> {
    const QUERY: &str = r#"
        INSERT INTO auction_transaction (auction_id, tx_from, tx_nonce)
        VALUES ($1, $2, $3)
        ON CONFLICT (auction_id) DO NOTHING
    "#;

    let result = sqlx::query(QUERY)
        .bind(auction_id)
        .bind(tx_from)
        .bind(tx_nonce)
        .execute(ex)
        .await?;

    Ok(result.rows_affected() == 1)
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

pub async fn get_auction_id(
    ex: &mut PgConnection,
    tx_from: &Address,
    tx_nonce: i64,
) -> Result<Option<AuctionId>, sqlx::Error> {
    const QUERY: &str =
        r#"SELECT auction_id FROM auction_transaction WHERE tx_from = $1 AND tx_nonce = $2;"#;
    let auction = sqlx::query_scalar(QUERY)
        .bind(tx_from)
        .bind(tx_nonce)
        .fetch_optional(ex)
        .await?;
    Ok(auction)
}

pub async fn data_exists(ex: &mut PgConnection, auction_id: i64) -> Result<bool, sqlx::Error> {
    const QUERY: &str = r#"SELECT COUNT(*) FROM auction_transaction WHERE auction_id = $1;"#;
    let count: i64 = sqlx::query_scalar(QUERY)
        .bind(auction_id)
        .fetch_one(ex)
        .await?;
    Ok(count >= 1)
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::events::{Event, EventIndex, Settlement},
        sqlx::Connection,
        std::ops::DerefMut,
    };

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
            .fetch_one(db.deref_mut())
            .await
            .unwrap();
        assert_eq!(auction_id, 1);

        // reusing auction-id fails
        let result = upsert_auction_transaction(&mut db, 1, &Default::default(), 1).await;
        assert!(result.is_err());
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
            .fetch_one(db.deref_mut())
            .await
            .unwrap();
        assert_eq!(auction_id, None);

        insert_settlement_tx_info(&mut db, 0, 0, &Default::default(), 1)
            .await
            .unwrap();

        let auction_id: Option<i64> = sqlx::query_scalar("SELECT tx_nonce FROM settlements")
            .fetch_one(db.deref_mut())
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
    async fn get_auction_id_test() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        upsert_auction_transaction(&mut db, 5, &Default::default(), 3)
            .await
            .unwrap();

        let auction_id = get_auction_id(&mut db, &Default::default(), 3)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(auction_id, 5);
    }

    #[tokio::test]
    #[ignore]
    async fn try_insert_auction_transaction_test() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let inserted = try_insert_auction_transaction(&mut db, 3, &Default::default(), 1)
            .await
            .unwrap();
        assert!(inserted);

        let inserted = try_insert_auction_transaction(&mut db, 3, &Default::default(), 1)
            .await
            .unwrap();
        assert!(!inserted);
    }
}
