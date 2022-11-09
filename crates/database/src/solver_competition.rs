use crate::{auction::AuctionId, TransactionHash};
use sqlx::{types::JsonValue, PgConnection};

// TODO: once the auction_transaction are verified to work we can remove the tx_hash
// column and argument.
pub async fn save(
    ex: &mut PgConnection,
    id: AuctionId,
    data: &JsonValue,
    tx_hash: Option<&TransactionHash>,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO solver_competitions (id, json, tx_hash)
VALUES ($1, $2, $3)
ON CONFLICT (id) DO UPDATE
SET json = EXCLUDED.json, tx_hash = EXCLUDED.tx_hash
    ;"#;
    sqlx::query(QUERY)
        .bind(id)
        .bind(data)
        .bind(tx_hash)
        .execute(ex)
        .await?;
    Ok(())
}

#[derive(sqlx::FromRow)]
pub struct LoadById {
    pub json: JsonValue,
    pub tx_hash: Option<TransactionHash>,
}

pub async fn load_by_id(
    ex: &mut PgConnection,
    id: AuctionId,
) -> Result<Option<LoadById>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, s.tx_hash
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN auction_transaction at ON sc.id = at.auction_id
LEFT OUTER JOIN settlements s ON (at.tx_from, at.tx_nonce) = (s.tx_from, s.tx_nonce)
WHERE sc.id = $1
    ;"#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

#[derive(sqlx::FromRow)]
pub struct LoadByTxHash {
    pub json: JsonValue,
    pub id: AuctionId,
}

pub async fn load_by_tx_hash(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Option<LoadByTxHash>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id
FROM solver_competitions sc
JOIN auction_transaction at ON sc.id = at.auction_id
JOIN settlements s ON (at.tx_from, at.tx_nonce) = (s.tx_from, s.tx_nonce)
WHERE s.tx_hash = $1
    ;"#;
    sqlx::query_as(QUERY).bind(tx_hash).fetch_optional(ex).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        byte_array::ByteArray,
        events::{Event, EventIndex, Settlement},
        Address,
    };
    use sqlx::Connection;

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Bool(true);
        save(&mut db, 0, &value, None).await.unwrap();
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());

        assert!(load_by_id(&mut db, 1).await.unwrap().is_none());

        let value = JsonValue::String("a".to_string());
        let tx_hash = ByteArray([0x01; 32]);
        save(&mut db, 0, &value, Some(&tx_hash)).await.unwrap();
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_by_hash() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let id: i64 = 5;
        let value = JsonValue::Bool(true);
        let hash = ByteArray([1u8; 32]);
        save(&mut db, id, &value, Some(&hash)).await.unwrap();

        let value_by_id = load_by_id(&mut db, id).await.unwrap().unwrap();
        assert_eq!(value, value_by_id.json);
        // no hash because hash columns isn't used to find it
        assert!(value_by_id.tx_hash.is_none());

        // Fails because the tx_hash stored directly in the solver_competitions table is no longer
        // used to look the competition up.
        assert!(load_by_tx_hash(&mut db, &hash).await.unwrap().is_none());

        // Now insert the proper settlement event and account-nonce.

        let index = EventIndex::default();
        let event = Event::Settlement(Settlement {
            solver: Default::default(),
            transaction_hash: hash,
        });
        crate::events::append(&mut db, &[(index, event)])
            .await
            .unwrap();

        let tx_from: Address = ByteArray([0x01; 20]);
        let tx_nonce: i64 = 2;
        crate::auction_transaction::insert_settlement_tx_info(
            &mut db,
            index.block_number,
            index.log_index,
            &tx_from,
            tx_nonce,
        )
        .await
        .unwrap();

        crate::auction_transaction::upsert_auction_transaction(&mut db, id, &tx_from, tx_nonce)
            .await
            .unwrap();

        // Now succeeds.
        let value_by_hash = load_by_tx_hash(&mut db, &hash).await.unwrap().unwrap();
        assert_eq!(value, value_by_hash.json);
        assert_eq!(id, value_by_hash.id);

        // By id also sees the hash now.
        let value_by_id = load_by_id(&mut db, id).await.unwrap().unwrap();
        assert_eq!(hash, value_by_id.tx_hash.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn postgres_can_overwrite() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Bool(true);
        save(&mut db, 0, &value, None).await.unwrap();
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());

        // overwrite id
        let value = JsonValue::Bool(false);
        let hash = ByteArray([1u8; 32]);
        save(&mut db, 0, &value, Some(&hash)).await.unwrap();
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());
    }
}
