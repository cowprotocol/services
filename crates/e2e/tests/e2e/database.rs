//! This module contains helper functions to query the state of the database
//! during a test.

use {
    crate::setup::Db,
    database::{byte_array::ByteArray, order_events, Address, TransactionHash},
    futures::TryStreamExt,
    model::order::OrderUid,
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

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AuctionTransaction {
    pub tx_from: Address,
    pub tx_nonce: i64,
    pub tx_hash: TransactionHash,
    pub block_number: i64,
    pub solver: Address,
    // index of the `Settlement` event
    pub log_index: i64,
}

#[derive(Clone, Debug)]
pub struct Cip20Data {
    pub observation: database::settlement_observations::Observation,
    pub tx: AuctionTransaction,
    pub participants: Vec<database::auction_participants::Participant>,
    pub prices: Vec<database::auction_prices::AuctionPrice>,
    pub score: database::settlement_scores::Score,
    pub trades: Vec<database::orders::OrderExecution>,
    // TODO add this when we eventually store the competition data
    // pub competition: serde_json::Value,
}

/// Returns `Some(data)` if the all the expected CIP-20 data has been indexed
/// for the most recent `auction_transaction`.
pub async fn most_recent_cip_20_data(db: &Db) -> Option<Cip20Data> {
    let mut db = db.acquire().await.unwrap();

    const LAST_AUCTION_ID: &str =
        "SELECT auction_id FROM auction_transaction ORDER BY auction_id DESC LIMIT 1";
    let auction_id: i64 = sqlx::query_scalar(LAST_AUCTION_ID)
        .fetch_optional(db.deref_mut())
        .await
        .unwrap()?;

    const TX_QUERY: &str = r"
SELECT s.*
FROM auction_transaction at
JOIN settlements s ON s.tx_from = at.tx_from AND s.tx_nonce = at.tx_nonce
WHERE at.auction_id = $1
    ";
    let tx: AuctionTransaction = sqlx::query_as(TX_QUERY)
        .bind(auction_id)
        .fetch_optional(db.deref_mut())
        .await
        .unwrap()?;

    let observation = database::settlement_observations::fetch(&mut db, &tx.tx_hash)
        .await
        .unwrap()?;
    let participants = database::auction_participants::fetch(&mut db, auction_id)
        .await
        .unwrap();
    let prices = database::auction_prices::fetch(&mut db, auction_id)
        .await
        .unwrap();
    let score = database::settlement_scores::fetch(&mut db, auction_id)
        .await
        .unwrap()?;
    let trades = database::orders::order_executions_in_tx(&mut db, &tx.tx_hash, auction_id)
        .try_collect()
        .await
        .ok()?;
    // TODO add this when we eventually store the competition data
    // let competition = database::solver_competition::load_by_id(&mut db,
    // auction_id)     .await
    //     .unwrap()?
    //     .json;

    Some(Cip20Data {
        observation,
        tx,
        participants,
        prices,
        score,
        trades,
        // competition,
    })
}
