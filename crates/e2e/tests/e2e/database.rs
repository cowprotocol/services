//! This module contains helper functions to query the state of the database
//! during a test.

use {
    database::{Address, TransactionHash, byte_array::ByteArray, order_events},
    e2e::setup::Db,
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

#[allow(dead_code)]
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct AuctionTransaction {
    pub tx_hash: TransactionHash,
    pub block_number: i64,
    pub solver: Address,
    // index of the `Settlement` event
    pub log_index: i64,
    pub solution_uid: i64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Cip20Data {
    pub observations: Vec<database::settlement_observations::Observation>,
    pub txs: Vec<AuctionTransaction>,
    pub participants: Vec<database::auction_participants::Participant>,
    pub prices: Vec<database::auction_prices::AuctionPrice>,
    pub reference_scores: Vec<database::reference_scores::Score>,
    pub competition: serde_json::Value,
}

/// Returns `Some(data)` if the all the expected CIP-20 data has been indexed
/// for the most recent `auction_id` from `settlements` table.
pub async fn most_recent_cip_20_data(db: &Db) -> Option<Cip20Data> {
    let mut db = db.acquire().await.unwrap();

    const LAST_AUCTION_ID: &str = "SELECT auction_id FROM settlements WHERE auction_id IS NOT \
                                   NULL ORDER BY auction_id DESC LIMIT 1";
    let auction_id: i64 = sqlx::query_scalar(LAST_AUCTION_ID)
        .fetch_optional(db.deref_mut())
        .await
        .unwrap()?;

    const TX_QUERY: &str = r"
SELECT * FROM settlements WHERE auction_id = $1";

    let txs: Vec<AuctionTransaction> = sqlx::query_as(TX_QUERY)
        .bind(auction_id)
        .fetch_all(db.deref_mut())
        .await
        .ok()?;

    let observations = database::settlement_observations::fetch(
        &mut db,
        &txs.iter().map(|tx| tx.tx_hash).collect::<Vec<_>>(),
    )
    .await
    .ok()?;
    let participants = database::auction_participants::fetch(&mut db, auction_id)
        .await
        .unwrap();
    let prices = database::auction_prices::fetch(&mut db, auction_id)
        .await
        .unwrap();
    let reference_scores = database::reference_scores::fetch(&mut db, auction_id)
        .await
        .unwrap();
    let competition = database::solver_competition::load_by_id(&mut db, auction_id)
        .await
        .unwrap()?
        .json;

    Some(Cip20Data {
        observations,
        txs,
        participants,
        prices,
        reference_scores,
        competition,
    })
}
