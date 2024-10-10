use {
    crate::{auction::AuctionId, TransactionHash},
    sqlx::{types::JsonValue, PgConnection},
};

pub async fn save(
    ex: &mut PgConnection,
    id: AuctionId,
    data: &JsonValue,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
INSERT INTO solver_competitions (id, json)
VALUES ($1, $2)
    ;"#;
    sqlx::query(QUERY).bind(id).bind(data).execute(ex).await?;
    Ok(())
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct LoadCompetition {
    pub json: JsonValue,
    pub id: AuctionId,
    // Multiple settlements can be associated with a single competition.
    pub tx_hash: Option<Vec<TransactionHash>>,
}

pub async fn load_by_id(
    ex: &mut PgConnection,
    id: AuctionId,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, ARRAY_AGG(s.tx_hash) FILTER (WHERE s.tx_hash IS NOT NULL) AS tx_hash
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
WHERE sc.id = $1
GROUP BY sc.json, sc.id
    ;"#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

pub async fn load_latest_competition(
    ex: &mut PgConnection,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, ARRAY_AGG(s.tx_hash) FILTER (WHERE s.tx_hash IS NOT NULL) AS tx_hash
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
GROUP BY sc.json, sc.id
ORDER BY sc.id DESC
LIMIT 1
    ;"#;
    sqlx::query_as(QUERY).fetch_optional(ex).await
}

pub async fn load_by_tx_hash(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
WITH competition AS (
    SELECT sc.id
    FROM solver_competitions sc
    JOIN settlements s ON sc.id = s.auction_id
    WHERE s.tx_hash = $1
)
SELECT sc.json, sc.id, ARRAY_AGG(s.tx_hash) FILTER (WHERE s.tx_hash IS NOT NULL) AS tx_hash
FROM solver_competitions sc
JOIN settlements s ON sc.id = s.auction_id
WHERE sc.id = (SELECT id FROM competition)
GROUP BY sc.json, sc.id
    ;"#;
    sqlx::query_as(QUERY).bind(tx_hash).fetch_optional(ex).await
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            byte_array::ByteArray,
            events::{EventIndex, Settlement},
        },
        sqlx::Connection,
    };

    #[tokio::test]
    #[ignore]
    async fn postgres_roundtrip() {
        let mut db = PgConnection::connect("postgresql://").await.unwrap();
        let mut db = db.begin().await.unwrap();
        crate::clear_DANGER_(&mut db).await.unwrap();

        let value = JsonValue::Bool(true);
        save(&mut db, 0, &value).await.unwrap();

        // load by id works
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());
        // load as latest works
        let value_ = load_latest_competition(&mut db).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hash.is_none());
        // load by tx doesn't work, as there is no settlement yet
        assert!(load_by_tx_hash(&mut db, &ByteArray([0u8; 32]))
            .await
            .unwrap()
            .is_none());

        // non-existent auction returns none
        assert!(load_by_id(&mut db, 1).await.unwrap().is_none());

        // insert two settlement events for the same auction id
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 0,
                log_index: 0,
            },
            &Settlement {
                solver: Default::default(),
                transaction_hash: ByteArray([0u8; 32]),
            },
        )
        .await
        .unwrap();
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 0,
                log_index: 1,
            },
            &Settlement {
                solver: Default::default(),
                transaction_hash: ByteArray([1u8; 32]),
            },
        )
        .await
        .unwrap();
        crate::settlements::update_settlement_auction(&mut db, 0, 0, 0)
            .await
            .unwrap();
        crate::settlements::update_settlement_auction(&mut db, 0, 1, 0)
            .await
            .unwrap();

        // load by id works, and finds two hashes
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert!(value_.tx_hash.unwrap().len() == 2);

        // load as latest works, and finds two hashes
        let value_ = load_latest_competition(&mut db).await.unwrap().unwrap();
        assert!(value_.tx_hash.unwrap().len() == 2);

        // load by tx works, and finds two hashes, no matter which tx hash is used
        let value_ = load_by_tx_hash(&mut db, &ByteArray([0u8; 32]))
            .await
            .unwrap()
            .unwrap();
        assert!(value_.tx_hash.unwrap().len() == 2);
        let value_ = load_by_tx_hash(&mut db, &ByteArray([1u8; 32]))
            .await
            .unwrap()
            .unwrap();
        assert!(value_.tx_hash.unwrap().len() == 2);
    }
}
