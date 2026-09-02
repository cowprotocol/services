//! This module contains helper functions to query the state of the database
//! during a test.

use {
    database::{Address, byte_array::ByteArray, order_events},
    e2e::setup::Db,
    model::order::OrderUid,
    sqlx::PgConnection,
    std::ops::DerefMut,
};

/// Returns all events of that order in the order they happend (old to new).
pub async fn events_of_order(db: &Db, uid: &OrderUid) -> Vec<order_events::OrderEvent> {
    const QUERY: &str = "SELECT * FROM order_events WHERE order_uid = $1 ORDER BY timestamp ASC";
    let mut db = db.acquire().await.unwrap();
    sqlx::query_as(QUERY)
        .bind(ByteArray(uid.0))
        .fetch_all(db.deref_mut())
        .await
        .unwrap()
}

/// Returns the persisted penalty caps of that order across all auctions.
pub async fn penalty_caps_of_order(db: &Db, uid: &OrderUid) -> Vec<bigdecimal::BigDecimal> {
    const QUERY: &str = r#"
SELECT penalty_caps_native[array_position(order_uids, $1)]
FROM competition_auctions
WHERE order_uids @> ARRAY[$1::bytea]
    "#;
    let mut db = db.acquire().await.unwrap();
    let rows: Vec<(Option<bigdecimal::BigDecimal>,)> = sqlx::query_as(QUERY)
        .bind(ByteArray(uid.0))
        .fetch_all(db.deref_mut())
        .await
        .unwrap();
    rows.into_iter().filter_map(|(cap,)| cap).collect()
}

/// Returns quote.
pub async fn quote_metadata(db: &Db, quote_id: i64) -> Option<(serde_json::Value,)> {
    const QUERY: &str = "SELECT metadata FROM quotes WHERE id = $1";
    let mut db = db.acquire().await.unwrap();
    sqlx::query_as(QUERY)
        .bind(quote_id)
        .fetch_optional(db.deref_mut())
        .await
        .unwrap()
}

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AuctionTransaction {
    pub solver: Address,
    pub solution_uid: i64,
}

pub async fn auction_participants(
    ex: &mut PgConnection,
    auction_id: i64,
) -> anyhow::Result<Vec<Address>> {
    const QUERY: &str = r#"
        SELECT DISTINCT ps.solver
        FROM proposed_solutions ps
        WHERE ps.auction_id = $1
    "#;
    Ok(sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?)
}

pub async fn auction_prices(
    ex: &mut PgConnection,
    auction_id: i64,
) -> anyhow::Result<Vec<database::auction_prices::AuctionPrice>> {
    const QUERY: &str = "
        SELECT
            id AS auction_id,
            unnest(price_tokens) AS token,
            unnest(price_values) AS price
        FROM competition_auctions WHERE id = $1";
    Ok(sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?)
}

pub async fn reference_scores(
    ex: &mut PgConnection,
    auction_id: i64,
) -> anyhow::Result<Vec<database::reference_scores::Score>> {
    const QUERY: &str = "SELECT * FROM reference_scores WHERE auction_id = $1";
    Ok(sqlx::query_as(QUERY).bind(auction_id).fetch_all(ex).await?)
}

pub async fn latest_auction_id(ex: &mut PgConnection) -> anyhow::Result<Option<i64>> {
    const QUERY: &str = "SELECT auction_id FROM settlements WHERE auction_id IS NOT NULL ORDER BY \
                         auction_id DESC LIMIT 1";
    Ok(sqlx::query_scalar(QUERY).fetch_optional(ex).await?)
}
