//! This file contains all functions related to reading or updating
//! data about the competition during an auction in the old JSON based
//! table.
//! See `solver_competition_v2.rs` for the new version of this which
//! uses individual and well defined tables for this.

use {
    crate::{TransactionHash, auction::AuctionId},
    sqlx::{PgConnection, types::JsonValue},
    tracing::instrument,
};

#[instrument(skip_all)]
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
    pub tx_hashes: Vec<TransactionHash>,
}

#[instrument(skip_all)]
pub async fn load_by_id(
    ex: &mut PgConnection,
    id: AuctionId,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, COALESCE(ARRAY_AGG(s.tx_hash) FILTER (WHERE so.block_number IS NOT NULL), '{}') AS tx_hashes
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
-- exclude settlements from another environment for which observation is guaranteed to not exist
LEFT OUTER JOIN settlement_observations so 
    ON s.block_number = so.block_number 
    AND s.log_index = so.log_index
WHERE sc.id = $1
GROUP BY sc.id
    ;"#;
    sqlx::query_as(QUERY).bind(id).fetch_optional(ex).await
}

#[instrument(skip_all)]
pub async fn load_latest_competitions(
    ex: &mut PgConnection,
    latest_competitions_count: u32,
) -> Result<Vec<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT sc.json, sc.id, COALESCE(ARRAY_AGG(s.tx_hash) FILTER (WHERE so.block_number IS NOT NULL), '{}') AS tx_hashes
FROM solver_competitions sc
-- outer joins because the data might not have been indexed yet
LEFT OUTER JOIN settlements s ON sc.id = s.auction_id
-- exclude settlements from another environment for which observation is guaranteed to not exist
LEFT OUTER JOIN settlement_observations so 
    ON s.block_number = so.block_number 
    AND s.log_index = so.log_index
GROUP BY sc.id
ORDER BY sc.id DESC
LIMIT $1
    ;"#;
    sqlx::query_as(QUERY)
        .bind(i64::from(latest_competitions_count))
        .fetch_all(ex)
        .await
}

pub async fn load_latest_competition(
    ex: &mut PgConnection,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    let competitions = load_latest_competitions(ex, 1).await?;
    let latest = competitions.into_iter().next();
    Ok(latest)
}

#[instrument(skip_all)]
pub async fn load_by_tx_hash(
    ex: &mut PgConnection,
    tx_hash: &TransactionHash,
) -> Result<Option<LoadCompetition>, sqlx::Error> {
    const QUERY: &str = r#"
WITH competition AS (
    SELECT sc.id
    FROM solver_competitions sc
    JOIN settlements s ON sc.id = s.auction_id
    JOIN settlement_observations so 
        ON s.block_number = so.block_number 
        AND s.log_index = so.log_index
    WHERE s.tx_hash = $1
)
SELECT sc.json, sc.id, COALESCE(ARRAY_AGG(s.tx_hash) FILTER (WHERE so.block_number IS NOT NULL), '{}') AS tx_hashes
FROM solver_competitions sc
JOIN settlements s ON sc.id = s.auction_id
JOIN settlement_observations so 
    ON s.block_number = so.block_number 
    AND s.log_index = so.log_index
WHERE sc.id = (SELECT id FROM competition)
GROUP BY sc.id
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
        assert!(value_.tx_hashes.is_empty());
        // load as latest works
        let value_ = load_latest_competition(&mut db).await.unwrap().unwrap();
        assert_eq!(value, value_.json);
        assert!(value_.tx_hashes.is_empty());
        // load by tx doesn't work, as there is no settlement yet
        assert!(
            load_by_tx_hash(&mut db, &ByteArray([0u8; 32]))
                .await
                .unwrap()
                .is_none()
        );

        // non-existent auction returns none
        assert!(load_by_id(&mut db, 1).await.unwrap().is_none());

        // insert three settlement events for the same auction id, with one of them not
        // having observation (in practice usually meaning it's from different
        // environment)
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
        crate::settlement_observations::upsert(
            &mut db,
            crate::settlement_observations::Observation {
                block_number: 0,
                log_index: 0,
                ..Default::default()
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
        crate::settlement_observations::upsert(
            &mut db,
            crate::settlement_observations::Observation {
                block_number: 0,
                log_index: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        crate::events::insert_settlement(
            &mut db,
            &EventIndex {
                block_number: 0,
                log_index: 2,
            },
            &Settlement {
                solver: Default::default(),
                transaction_hash: ByteArray([2u8; 32]),
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
        crate::settlements::update_settlement_auction(&mut db, 0, 2, 0)
            .await
            .unwrap();

        // load by id works, and finds two hashes
        let value_ = load_by_id(&mut db, 0).await.unwrap().unwrap();
        assert!(value_.tx_hashes.len() == 2);

        // load as latest works, and finds two hashes
        let value_ = load_latest_competition(&mut db).await.unwrap().unwrap();
        assert!(value_.tx_hashes.len() == 2);

        // load by tx works, and finds two hashes, no matter which tx hash is used
        let value_ = load_by_tx_hash(&mut db, &ByteArray([0u8; 32]))
            .await
            .unwrap()
            .unwrap();
        assert!(value_.tx_hashes.len() == 2);
        let value_ = load_by_tx_hash(&mut db, &ByteArray([1u8; 32]))
            .await
            .unwrap()
            .unwrap();
        assert!(value_.tx_hashes.len() == 2);
        // this one should not find any hashes since it's from another environment
        let value_ = load_by_tx_hash(&mut db, &ByteArray([2u8; 32]))
            .await
            .unwrap();
        assert!(value_.is_none());
    }
}
