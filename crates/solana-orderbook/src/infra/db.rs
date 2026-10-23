//! Database access for the Solana orderbook.

use {
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        solana::{OrderEventLabel, OrderKind},
    },
    sqlx::{Connection, PgExecutor, PgPool},
};

/// One order joined with its fill state.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct OrderRow {
    pub uid: ByteArray<32>,
    pub owner: ByteArray<32>,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_token_account: ByteArray<32>,
    pub buy_token_account: ByteArray<32>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub valid_to: i64,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub app_data: ByteArray<32>,
    pub creation_timestamp: DateTime<Utc>,
    pub order_pda: ByteArray<32>,
    pub amount_withdrawn: BigDecimal,
    pub amount_received: BigDecimal,
    pub cancellation_timestamp: Option<DateTime<Utc>>,
    /// The height the stored creation transaction dies at, while the order
    /// still awaits its on-chain creation. `None` once created, and for
    /// orders that were never sponsored.
    pub last_valid_block_height: Option<i64>,
}

/// Read one order with its fill state. `None` when the uid is unknown.
pub async fn find_order_by_uid(ex: impl PgExecutor<'_>, uid: [u8; 32]) -> Result<Option<OrderRow>> {
    let _timer = query_timer("find_order_by_uid");
    const QUERY: &str = r#"
SELECT o.uid, o.owner, o.sell_token, o.buy_token, o.sell_token_account,
       o.buy_token_account, o.sell_amount, o.buy_amount, o.valid_to,
       o.kind, o.partially_fillable, o.app_data,
       o.creation_timestamp, o.order_pda,
       COALESCE(p.amount_withdrawn, 0) AS amount_withdrawn,
       COALESCE(p.amount_received, 0) AS amount_received,
       p.cancellation_timestamp,
       CASE WHEN o.presigned_transaction IS NOT NULL AND p.order_uid IS NULL
            THEN o.last_valid_block_height END AS last_valid_block_height
FROM solana.orders o
LEFT JOIN solana.order_pda p ON p.order_uid = o.uid
WHERE o.uid = $1 AND NOT COALESCE(p.is_reorged, false)
    "#;
    sqlx::query_as(QUERY)
        .bind(ByteArray(uid))
        .fetch_optional(ex)
        .await
        .context("read solana.orders by uid")
}

/// A page of one owner's orders with their fill state, newest first.
pub async fn get_orders_by_owner(
    ex: impl PgExecutor<'_>,
    owner: [u8; 32],
    offset: i64,
    limit: i64,
) -> Result<Vec<OrderRow>> {
    let _timer = query_timer("get_orders_by_owner");
    const QUERY: &str = r#"
SELECT o.uid, o.owner, o.sell_token, o.buy_token, o.sell_token_account,
       o.buy_token_account, o.sell_amount, o.buy_amount, o.valid_to,
       o.kind, o.partially_fillable, o.app_data,
       o.creation_timestamp, o.order_pda,
       COALESCE(p.amount_withdrawn, 0) AS amount_withdrawn,
       COALESCE(p.amount_received, 0) AS amount_received,
       p.cancellation_timestamp,
       CASE WHEN o.presigned_transaction IS NOT NULL AND p.order_uid IS NULL
            THEN o.last_valid_block_height END AS last_valid_block_height
FROM solana.orders o
LEFT JOIN solana.order_pda p ON p.order_uid = o.uid
WHERE o.owner = $1
ORDER BY o.creation_timestamp DESC
LIMIT $2 OFFSET $3
    "#;
    sqlx::query_as(QUERY)
        .bind(ByteArray(owner))
        .bind(limit)
        .bind(offset)
        .fetch_all(ex)
        .await
        .context("read solana.orders by owner")
}

/// One trade joined with its order's identity and the settlement's slot.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct TradeRow {
    pub order_uid: ByteArray<32>,
    pub owner: ByteArray<32>,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub tx_signature: ByteArray<64>,
    pub instruction_index: i32,
    pub slot: Option<i64>,
}

/// Trades filtered by order uid or owner, newest first, paged by `offset`
/// and `limit`. The slot comes from any settlement row of the transaction
/// because the slot is constant per transaction. A trade whose order row is
/// not indexed yet is omitted until the order lands.
pub async fn get_trades(
    ex: impl PgExecutor<'_>,
    order_uid: Option<[u8; 32]>,
    owner: Option<[u8; 32]>,
    offset: i64,
    limit: i64,
) -> Result<Vec<TradeRow>> {
    let _timer = query_timer("get_trades");
    const QUERY: &str = r#"
SELECT t.order_uid, o.owner, o.sell_token, o.buy_token,
       t.sell_amount, t.buy_amount, t.tx_signature,
       t.instruction_index, s.slot
FROM solana.trades t
JOIN solana.orders o ON o.uid = t.order_uid
LEFT JOIN LATERAL (
    SELECT slot FROM solana.settlements
    WHERE tx_signature = t.tx_signature
    LIMIT 1
) s ON true
WHERE ($1::bytea IS NULL OR t.order_uid = $1)
  AND ($2::bytea IS NULL OR o.owner = $2)
ORDER BY s.slot DESC NULLS LAST, t.tx_signature DESC, t.instruction_index DESC,
         t.order_uid DESC
OFFSET $3 LIMIT $4
    "#;
    sqlx::query_as(QUERY)
        .bind(order_uid.map(ByteArray))
        .bind(owner.map(ByteArray))
        .bind(offset)
        .bind(limit)
        .fetch_all(ex)
        .await
        .context("read solana.trades")
}

/// Whether the order has at least one trade.
pub async fn order_has_trade(ex: impl PgExecutor<'_>, uid: [u8; 32]) -> Result<bool> {
    let _timer = query_timer("order_has_trade");
    sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM solana.trades WHERE order_uid = $1)",
    )
    .bind(ByteArray(uid))
    .fetch_one(ex)
    .await
    .context("check solana.trades existence")
}

/// A sponsored order as accepted by the placement endpoint: the intent
/// fields plus the user's partially signed `CreateOrder` transaction and
/// the block height at which that transaction dies with its blockhash.
#[derive(Clone, Debug)]
pub struct SponsoredOrder {
    pub uid: ByteArray<32>,
    pub owner: ByteArray<32>,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_token_account: ByteArray<32>,
    pub buy_token_account: ByteArray<32>,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub valid_to: u32,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub app_data: ByteArray<32>,
    pub order_pda: ByteArray<32>,
    pub presigned_transaction: Vec<u8>,
    pub last_valid_block_height: u64,
}

/// The quote an order was placed against, copied under the order's uid so
/// the record outlives any cleanup of `solana.quotes`.
#[derive(Clone, Debug)]
pub struct OrderQuote {
    pub quote_id: i64,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub solver: ByteArray<32>,
}

/// Whether an order with this uid is already stored.
pub async fn order_exists(pool: &PgPool, uid: &[u8; 32]) -> Result<bool> {
    let _timer = query_timer("order_exists");
    sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM solana.orders WHERE uid = $1)")
        .bind(ByteArray(*uid))
        .fetch_one(pool)
        .await
        .context("check solana.orders existence")
}

/// Insert a sponsored order, its quote copy when one matched, and its
/// `created` event in one transaction. A duplicate uid or order PDA surfaces
/// as a unique violation.
pub async fn insert_sponsored_order(
    pool: &PgPool,
    order: &SponsoredOrder,
    quote: Option<&OrderQuote>,
) -> Result<()> {
    let _timer = query_timer("insert_sponsored_order");
    const QUERY: &str = r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda,
    presigned_transaction, last_valid_block_height)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, now(), $13, $14, $15)
    "#;
    let mut tx = pool.begin().await.context("begin sponsored order insert")?;
    sqlx::query(QUERY)
        .bind(order.uid)
        .bind(order.owner)
        .bind(order.sell_token)
        .bind(order.buy_token)
        .bind(order.sell_token_account)
        .bind(order.buy_token_account)
        .bind(BigDecimal::from(order.sell_amount))
        .bind(BigDecimal::from(order.buy_amount))
        .bind(i64::from(order.valid_to))
        .bind(order.kind)
        .bind(order.partially_fillable)
        .bind(order.app_data)
        .bind(order.order_pda)
        .bind(&order.presigned_transaction)
        .bind(i64::try_from(order.last_valid_block_height).context("block height exceeds i64")?)
        .execute(&mut *tx)
        .await
        .context("insert sponsored order")?;
    if let Some(quote) = quote {
        sqlx::query(
            "INSERT INTO solana.order_quotes (order_uid, quote_id, sell_amount, buy_amount, \
             solver) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(order.uid)
        .bind(quote.quote_id)
        .bind(&quote.sell_amount)
        .bind(&quote.buy_amount)
        .bind(quote.solver)
        .execute(&mut *tx)
        .await
        .context("insert order quote")?;
    }
    sqlx::query(
        "INSERT INTO solana.order_events (order_uid, timestamp, label) VALUES ($1, now(), $2)",
    )
    .bind(order.uid)
    .bind(OrderEventLabel::Created)
    .execute(&mut *tx)
    .await
    .context("insert created event")?;
    tx.commit().await.context("commit sponsored order insert")
}

/// A stored sponsored creation: the presigned transaction and its expiry.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct SponsoredCreation {
    pub presigned_transaction: Vec<u8>,
    pub last_valid_block_height: i64,
}

/// The sponsored creation of an order. `None` when the uid is unknown or
/// the order was not placed through the sponsored path.
pub async fn find_sponsored_creation(
    ex: impl PgExecutor<'_>,
    uid: [u8; 32],
) -> Result<Option<SponsoredCreation>> {
    let _timer = query_timer("find_sponsored_creation");
    sqlx::query_as(
        "SELECT presigned_transaction, last_valid_block_height
         FROM solana.orders
         WHERE uid = $1 AND presigned_transaction IS NOT NULL",
    )
    .bind(ByteArray(uid))
    .fetch_optional(ex)
    .await
    .context("read sponsored creation")
}

/// The label of the order's most recent auction-progress event.
pub async fn find_latest_order_event(
    ex: impl PgExecutor<'_>,
    uid: [u8; 32],
) -> Result<Option<OrderEventLabel>> {
    let _timer = query_timer("find_latest_order_event");
    const QUERY: &str = r#"
SELECT label
FROM solana.order_events
WHERE order_uid = $1
ORDER BY timestamp DESC
LIMIT 1
    "#;
    sqlx::query_scalar(QUERY)
        .bind(ByteArray(uid))
        .fetch_optional(ex)
        .await
        .context("read solana.order_events")
}

/// One proposed solution of the competition an order's status reports, with
/// the amounts it executes for the order when it includes the order.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct SolutionRow {
    pub solver: ByteArray<32>,
    pub executed_sell: Option<BigDecimal>,
    pub executed_buy: Option<BigDecimal>,
}

/// The proposed solutions of the auction that settled the order's latest
/// trade, or of the latest auction in which a solution executed the order.
/// Worst to best with the winner last, the order the EVM status endpoint
/// lists them in. Empty when no competition knows the order.
pub async fn find_order_solutions(
    ex: impl PgExecutor<'_>,
    uid: [u8; 32],
) -> Result<Vec<SolutionRow>> {
    let _timer = query_timer("find_order_solutions");
    const QUERY: &str = r#"
SELECT ps.solver, pte.executed_sell, pte.executed_buy
FROM solana.proposed_solutions ps
LEFT JOIN solana.proposed_trade_executions pte
  ON pte.auction_id = ps.auction_id AND pte.solution_uid = ps.uid AND pte.order_uid = $1
WHERE ps.auction_id = COALESCE(
    (SELECT s.auction_id
     FROM solana.trades t
     JOIN solana.settlements s
       ON s.tx_signature = t.tx_signature AND s.instruction_index = t.instruction_index
     WHERE t.order_uid = $1
     ORDER BY t.slot DESC, t.instruction_index DESC
     LIMIT 1),
    (SELECT max(auction_id) FROM solana.proposed_trade_executions WHERE order_uid = $1))
ORDER BY ps.uid DESC
    "#;
    sqlx::query_as(QUERY)
        .bind(ByteArray(uid))
        .fetch_all(ex)
        .await
        .context("read solana.proposed_solutions by order")
}

/// The auction a competition ranked, as snapshot at ranking time.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct CompetitionAuction {
    pub id: i64,
    pub tip_slot: i64,
    pub deadline_slot: i64,
    pub order_uids: Vec<ByteArray<32>>,
    pub price_tokens: Vec<ByteArray<32>>,
    pub price_values: Vec<BigDecimal>,
}

/// One proposed solution of a competition with its rank, its solver's
/// reference score, and the settlement that executed it once attributed.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct CompetitionSolution {
    pub uid: i64,
    pub ranking: i64,
    pub solver: ByteArray<32>,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub score: BigDecimal,
    pub reference_score: Option<BigDecimal>,
    pub tx_signature: Option<ByteArray<64>>,
}

/// One order execution a proposed solution contains, with the order's mints.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct CompetitionTrade {
    pub solution_uid: i64,
    pub order_uid: ByteArray<32>,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
}

/// A competition: its auction, solutions, executions, and settlements.
#[derive(Clone, Debug)]
pub struct Competition {
    pub auction: CompetitionAuction,
    /// Worst to best, the winner last.
    pub solutions: Vec<CompetitionSolution>,
    pub trades: Vec<CompetitionTrade>,
    /// The transactions that settled the auction, attributed to a solution.
    pub tx_signatures: Vec<ByteArray<64>>,
}

/// The competition of one auction. `None` for an unknown auction, and for
/// one whose deadline slot the indexer has not passed: its winners may still
/// submit their settlements, and the solutions would show what they trade.
pub async fn find_competition_by_id(pool: &PgPool, auction_id: i64) -> Result<Option<Competition>> {
    let _timer = query_timer("find_competition_by_id");
    read_competition(pool, auction_id, true).await
}

/// The competition of one auction, before its deadline too, for internal
/// callers only.
pub async fn find_competition_by_id_unfiltered(
    pool: &PgPool,
    auction_id: i64,
) -> Result<Option<Competition>> {
    let _timer = query_timer("find_competition_by_id_unfiltered");
    read_competition(pool, auction_id, false).await
}

async fn read_competition(
    pool: &PgPool,
    auction_id: i64,
    hide_before_deadline: bool,
) -> Result<Option<Competition>> {
    const AUCTION: &str = r#"
SELECT id, tip_slot, deadline_slot, order_uids, price_tokens, price_values
FROM solana.competition_auctions
WHERE id = $1 AND (NOT $2 OR deadline_slot <= (SELECT slot FROM solana.indexer_state))
    "#;
    // Solution uids count from 0 at the best, so `uid + 1` ranks them.
    const SOLUTIONS: &str = r#"
SELECT ps.uid, ps.uid + 1 AS ranking, ps.solver, ps.is_winner, ps.filtered_out, ps.score,
       rs.reference_score,
       (SELECT s.tx_signature
        FROM solana.settlements s
        WHERE s.auction_id = ps.auction_id AND s.solution_uid = ps.uid
        LIMIT 1) AS tx_signature
FROM solana.proposed_solutions ps
LEFT JOIN solana.reference_scores rs
  ON rs.auction_id = ps.auction_id AND rs.solver = ps.solver
WHERE ps.auction_id = $1
ORDER BY ps.uid DESC
    "#;
    const TRADES: &str = r#"
SELECT pte.solution_uid, pte.order_uid, pte.executed_sell, pte.executed_buy,
       o.sell_token, o.buy_token
FROM solana.proposed_trade_executions pte
JOIN solana.orders o ON o.uid = pte.order_uid
WHERE pte.auction_id = $1
ORDER BY pte.solution_uid, pte.order_uid
    "#;
    const SIGNATURES: &str = r#"
SELECT DISTINCT tx_signature
FROM solana.settlements
WHERE auction_id = $1 AND solution_uid IS NOT NULL
ORDER BY tx_signature
    "#;
    let auction: Option<CompetitionAuction> = sqlx::query_as(AUCTION)
        .bind(auction_id)
        .bind(hide_before_deadline)
        .fetch_optional(pool)
        .await
        .context("read solana.competition_auctions")?;
    let Some(auction) = auction else {
        return Ok(None);
    };
    let solutions = sqlx::query_as(SOLUTIONS)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.proposed_solutions")?;
    let trades = sqlx::query_as(TRADES)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.proposed_trade_executions")?;
    let tx_signatures = sqlx::query_scalar(SIGNATURES)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.settlements by auction")?;
    Ok(Some(Competition {
        auction,
        solutions,
        trades,
        tx_signatures,
    }))
}

/// The competition the transaction settled, under the visibility rule of
/// [`find_competition_by_id`]. `None` when no settlement attributed to a
/// solution carries the signature.
pub async fn find_competition_by_tx_signature(
    pool: &PgPool,
    tx_signature: [u8; 64],
) -> Result<Option<Competition>> {
    let _timer = query_timer("find_competition_by_tx_signature");
    let auction_id: Option<i64> = sqlx::query_scalar(
        "SELECT auction_id FROM solana.settlements WHERE tx_signature = $1 AND solution_uid IS \
         NOT NULL LIMIT 1",
    )
    .bind(ByteArray(tx_signature))
    .fetch_optional(pool)
    .await
    .context("read solana.settlements by signature")?;
    match auction_id {
        Some(auction_id) => find_competition_by_id(pool, auction_id).await,
        None => Ok(None),
    }
}

/// The most recent competition whose deadline slot the indexer has passed.
pub async fn find_latest_competition(pool: &PgPool) -> Result<Option<Competition>> {
    let _timer = query_timer("find_latest_competition");
    const QUERY: &str = r#"
SELECT id
FROM solana.competition_auctions
WHERE deadline_slot <= (SELECT slot FROM solana.indexer_state)
ORDER BY id DESC
LIMIT 1
    "#;
    let auction_id: Option<i64> = sqlx::query_scalar(QUERY)
        .fetch_optional(pool)
        .await
        .context("read the latest solana.competition_auctions")?;
    match auction_id {
        Some(auction_id) => find_competition_by_id(pool, auction_id).await,
        None => Ok(None),
    }
}

/// A quote as the quote endpoint hands it out.
#[derive(Clone, Debug)]
pub struct Quote {
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_amount: u64,
    pub buy_amount: u64,
    pub kind: OrderKind,
    pub solver: ByteArray<32>,
    pub expiration: DateTime<Utc>,
}

/// Store a quote and return the id allocated for it.
pub async fn save_quote(ex: impl PgExecutor<'_>, quote: &Quote) -> Result<i64> {
    let _timer = query_timer("save_quote");
    const QUERY: &str = r#"
INSERT INTO solana.quotes (sell_token, buy_token, sell_amount, buy_amount,
    kind, solver, expiration_timestamp)
VALUES ($1, $2, $3, $4, $5, $6, $7)
RETURNING id
    "#;
    sqlx::query_scalar(QUERY)
        .bind(quote.sell_token)
        .bind(quote.buy_token)
        .bind(BigDecimal::from(quote.sell_amount))
        .bind(BigDecimal::from(quote.buy_amount))
        .bind(quote.kind)
        .bind(quote.solver)
        .bind(quote.expiration)
        .fetch_one(ex)
        .await
        .context("insert solana.quotes")
}

/// A stored quote, as read back to link an order against it.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct StoredQuote {
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub kind: OrderKind,
    pub solver: ByteArray<32>,
    pub expiration: DateTime<Utc>,
}

/// Read one stored quote. `None` when the id is unknown.
pub async fn read_quote(ex: impl PgExecutor<'_>, id: i64) -> Result<Option<StoredQuote>> {
    let _timer = query_timer("read_quote");
    const QUERY: &str = r#"
SELECT sell_token, buy_token, sell_amount, buy_amount, kind, solver,
       expiration_timestamp AS expiration
FROM solana.quotes
WHERE id = $1
    "#;
    sqlx::query_as(QUERY)
        .bind(id)
        .fetch_optional(ex)
        .await
        .context("read solana.quotes")
}

/// Check that the pool gives a connection and that the database answers on
/// it.
pub async fn ping(pool: &PgPool) -> Result<()> {
    let _timer = query_timer("ping");
    let mut conn = pool.acquire().await.context("acquire a connection")?;
    conn.ping().await.context("ping the database")
}

/// Starts timing a query, recorded under `query` when the timer drops.
fn query_timer(query: &str) -> prometheus::HistogramTimer {
    Metrics::instance(observe::metrics::get_storage_registry())
        .unwrap()
        .database_queries
        .with_label_values(&[query])
        .start_timer()
}

#[derive(prometheus_metric_storage::MetricStorage)]
struct Metrics {
    /// Query durations, by query.
    #[metric(labels("type"))]
    database_queries: prometheus::HistogramVec,
}

#[cfg(test)]
mod tests {
    use {super::*, sqlx::PgPool};

    async fn seed(pool: &PgPool, uid: [u8; 32], cancelled: bool) {
        sqlx::query(
            "TRUNCATE solana.order_pda, solana.orders, solana.trades, solana.settlements CASCADE",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda)
VALUES ($1, $2, $2, $3, $2, $2, 1000, 500, $4, 'sell'::solana.OrderKind, false, $2, now(), $5)
            "#,
        )
        .bind(ByteArray(uid))
        .bind(ByteArray([0xAA; 32]))
        .bind(ByteArray([0xAB; 32]))
        .bind(i64::from(u32::MAX))
        .bind(ByteArray([0xB0; 32]))
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
INSERT INTO solana.order_pda (order_uid, created_by, amount_withdrawn, cancellation_timestamp)
VALUES ($1, $2, 400, CASE WHEN $3 THEN now() END)
            "#,
        )
        .bind(ByteArray(uid))
        .bind(ByteArray([0xAA; 32]))
        .bind(cancelled)
        .execute(pool)
        .await
        .unwrap();
    }

    /// Pagination walks one owner's orders newest first, other owners are
    /// excluded, and the fill state joins in.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_orders_by_owner_paginated() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        seed(&pool, [0x11; 32], false).await;
        // A second, older order of the same owner, and one of another owner.
        for (uid, owner, age) in [
            ([0x12u8; 32], [0xAAu8; 32], "1 hour"),
            ([0x13; 32], [0xCC; 32], "2 hours"),
        ] {
            sqlx::query(
                r#"
INSERT INTO solana.orders (uid, owner, sell_token, buy_token, sell_token_account,
    buy_token_account, sell_amount, buy_amount, valid_to, kind,
    partially_fillable, app_data, creation_timestamp, order_pda)
VALUES ($1, $2, $2, $2, $2, $2, 1000, 500, $3, 'sell'::solana.OrderKind,
        false, $2, now() - $4::interval, $1)
                "#,
            )
            .bind(ByteArray(uid))
            .bind(ByteArray(owner))
            .bind(i64::from(u32::MAX))
            .bind(age)
            .execute(&pool)
            .await
            .unwrap();
        }

        let page = get_orders_by_owner(&pool, [0xAA; 32], 0, 10).await.unwrap();
        let uids: Vec<_> = page.iter().map(|row| row.uid).collect();
        assert_eq!(uids, vec![ByteArray([0x11; 32]), ByteArray([0x12; 32])]);
        assert_eq!(page[0].amount_withdrawn, BigDecimal::from(400));
        assert_eq!(page[1].amount_withdrawn, BigDecimal::from(0));

        let second = get_orders_by_owner(&pool, [0xAA; 32], 1, 1).await.unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].uid, ByteArray([0x12; 32]));
    }

    /// A sponsored order lands with its presigned transaction, expiry, and
    /// `created` event in one shot, and a duplicate uid is rejected.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_inserts_a_sponsored_order_once() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        sqlx::query(
            "TRUNCATE solana.order_pda, solana.orders, solana.order_quotes, solana.order_events \
             CASCADE",
        )
        .execute(&pool)
        .await
        .unwrap();
        let order = SponsoredOrder {
            uid: ByteArray([0x11; 32]),
            owner: ByteArray([0xAA; 32]),
            sell_token: ByteArray([0x66; 32]),
            buy_token: ByteArray([0x55; 32]),
            sell_token_account: ByteArray([0x33; 32]),
            buy_token_account: ByteArray([0x22; 32]),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: u32::MAX,
            kind: OrderKind::Sell,
            partially_fillable: false,
            app_data: ByteArray([0x44; 32]),
            order_pda: ByteArray([0xB0; 32]),
            presigned_transaction: vec![0xC0; 128],
            last_valid_block_height: 12_345,
        };
        let quote = OrderQuote {
            quote_id: 7,
            sell_amount: BigDecimal::from(1_000u64),
            buy_amount: BigDecimal::from(2_100u64),
            solver: ByteArray([0xDD; 32]),
        };
        insert_sponsored_order(&pool, &order, Some(&quote))
            .await
            .unwrap();

        let creation = find_sponsored_creation(&pool, order.uid.0)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(creation.presigned_transaction, vec![0xC0; 128]);
        assert_eq!(creation.last_valid_block_height, 12_345);
        let copied: (Vec<u8>, Option<i64>, String, Vec<u8>) = sqlx::query_as(
            "SELECT order_uid, quote_id, buy_amount::text, solver FROM solana.order_quotes",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(copied.0, order.uid.0.to_vec());
        assert_eq!(copied.1, Some(7));
        assert_eq!(copied.2, "2100");
        assert_eq!(copied.3, [0xDD; 32].to_vec());
        let events: Vec<(Vec<u8>, OrderEventLabel)> =
            sqlx::query_as("SELECT order_uid, label FROM solana.order_events")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert_eq!(
            events,
            vec![(order.uid.0.to_vec(), OrderEventLabel::Created)]
        );

        // The uid is the primary key: placing the same order twice fails and
        // leaves no second event behind.
        assert!(insert_sponsored_order(&pool, &order, None).await.is_err());
        let events: i64 = sqlx::query_scalar("SELECT count(*) FROM solana.order_events")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(events, 1);

        // An order indexed from chain has no sponsored creation.
        assert!(
            find_sponsored_creation(&pool, [0x99; 32])
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_an_order_with_fill_state() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;

        let row = find_order_by_uid(&pool, uid).await.unwrap().unwrap();
        assert_eq!(row.uid, ByteArray(uid));
        assert_eq!(row.kind, OrderKind::Sell);
        assert_eq!(row.amount_withdrawn, BigDecimal::from(400));
        assert_eq!(row.amount_received, BigDecimal::from(0));
        assert!(row.cancellation_timestamp.is_none());

        assert!(
            find_order_by_uid(&pool, [0x99; 32])
                .await
                .unwrap()
                .is_none()
        );

        seed(&pool, uid, true).await;
        let row = find_order_by_uid(&pool, uid).await.unwrap().unwrap();
        assert!(row.cancellation_timestamp.is_some());
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_order_events_and_existence() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;
        sqlx::query("TRUNCATE solana.order_events")
            .execute(&pool)
            .await
            .unwrap();

        assert!(!order_has_trade(&pool, uid).await.unwrap());
        assert!(find_latest_order_event(&pool, uid).await.unwrap().is_none());

        for (at, label) in [(1, OrderEventLabel::Ready), (2, OrderEventLabel::Executing)] {
            sqlx::query(
                "INSERT INTO solana.order_events (order_uid, timestamp, label) VALUES ($1, \
                 to_timestamp($2), $3)",
            )
            .bind(ByteArray(uid))
            .bind(at)
            .bind(label)
            .execute(&pool)
            .await
            .unwrap();
        }
        assert_eq!(
            find_latest_order_event(&pool, uid).await.unwrap(),
            Some(OrderEventLabel::Executing)
        );

        sqlx::query(
            "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid, sell_amount, \
             buy_amount, fee_amount, slot) VALUES ($1, 1, $2, 400, 200, 0, 42)",
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray(uid))
        .execute(&pool)
        .await
        .unwrap();
        assert!(order_has_trade(&pool, uid).await.unwrap());
    }

    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_trades_by_uid_and_owner() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;
        sqlx::query(
            "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid,              \
             sell_amount, buy_amount, fee_amount, slot) VALUES ($1, 1, $2, 400, 200, 0, 42)",
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray(uid))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver,              auction_id) VALUES (42, $1, 1, $2, 7)",
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray([0xCC; 32]))
        .execute(&pool)
        .await
        .unwrap();

        let by_uid = get_trades(&pool, Some(uid), None, 0, 10).await.unwrap();
        assert_eq!(by_uid.len(), 1);
        assert_eq!(by_uid[0].slot, Some(42));
        assert_eq!(by_uid[0].instruction_index, 1);
        assert_eq!(by_uid[0].sell_amount, BigDecimal::from(400));

        let by_owner = get_trades(&pool, None, Some([0xAA; 32]), 0, 10)
            .await
            .unwrap();
        assert_eq!(by_owner.len(), 1);

        assert!(
            get_trades(&pool, Some([0x99; 32]), None, 0, 10)
                .await
                .unwrap()
                .is_empty()
        );

        // A second fill of the same order pages: one row per page, newest
        // first, and an offset past the result set is empty.
        sqlx::query(
            "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid,              \
             sell_amount, buy_amount, fee_amount, slot) VALUES ($1, 2, $2, 100, 50, 0, 43)",
        )
        .bind(ByteArray([8u8; 64]))
        .bind(ByteArray(uid))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver,              auction_id) VALUES (43, $1, 2, $2, 8)",
        )
        .bind(ByteArray([8u8; 64]))
        .bind(ByteArray([0xCC; 32]))
        .execute(&pool)
        .await
        .unwrap();
        let first = get_trades(&pool, Some(uid), None, 0, 1).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].slot, Some(43));
        let second = get_trades(&pool, Some(uid), None, 1, 1).await.unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].slot, Some(42));
        assert!(
            get_trades(&pool, Some(uid), None, 2, 1)
                .await
                .unwrap()
                .is_empty()
        );
    }

    /// A competition reads back once the indexer passes its deadline slot:
    /// by auction id, by settlement signature, and as the latest. Solutions
    /// list worst to best with their executions, the reference score joins
    /// by solver, and an attributed settlement names its transaction.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_competitions() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;
        sqlx::query(
            "TRUNCATE solana.competition_auctions, solana.proposed_solutions, \
             solana.proposed_trade_executions, solana.reference_scores, solana.indexer_state",
        )
        .execute(&pool)
        .await
        .unwrap();
        // Auction 1 passed its deadline at the indexer's slot, auction 2
        // has not.
        sqlx::query("INSERT INTO solana.indexer_state (slot) VALUES (150)")
            .execute(&pool)
            .await
            .unwrap();
        for (auction, deadline) in [(1i64, 100i64), (2, 200)] {
            sqlx::query(
                "INSERT INTO solana.competition_auctions (id, tip_slot, deadline_slot, \
                 order_uids, price_tokens, price_values) VALUES ($1, $2 - 25, $2, $3, $4, $5)",
            )
            .bind(auction)
            .bind(deadline)
            .bind(vec![ByteArray(uid)])
            .bind(vec![ByteArray([0xAA; 32]), ByteArray([0xAB; 32])])
            .bind(vec![BigDecimal::from(5), BigDecimal::from(7)])
            .execute(&pool)
            .await
            .unwrap();
        }
        // (auction, solution uid, solver, winner, filtered out, score)
        for (auction, solution, solver, winner, filtered_out, score) in [
            (1i64, 0i64, 0xA1u8, true, false, 9i64),
            (1, 1, 0xA2, false, true, 5),
            (2, 0, 0xB1, true, false, 8),
        ] {
            sqlx::query(
                "INSERT INTO solana.proposed_solutions (auction_id, uid, id, solver, is_winner, \
                 filtered_out, score) VALUES ($1, $2, $2, $3, $4, $5, $6)",
            )
            .bind(auction)
            .bind(solution)
            .bind(ByteArray([solver; 32]))
            .bind(winner)
            .bind(filtered_out)
            .bind(score)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, \
                 order_uid, executed_sell, executed_buy) VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(auction)
            .bind(solution)
            .bind(ByteArray(uid))
            .bind(BigDecimal::from(score * 10))
            .bind(BigDecimal::from(score * 20))
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO solana.reference_scores (auction_id, solver, reference_score) VALUES (1, \
             $1, 4)",
        )
        .bind(ByteArray([0xA1; 32]))
        .execute(&pool)
        .await
        .unwrap();
        // The winner's settlement, attributed to its solution, and one not
        // attributed yet.
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, \
             auction_id, solution_uid) VALUES (90, $1, 1, $2, 1, 0)",
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray([0xA1; 32]))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, \
             auction_id) VALUES (91, $1, 1, $2, 1)",
        )
        .bind(ByteArray([7u8; 64]))
        .bind(ByteArray([0xA2; 32]))
        .execute(&pool)
        .await
        .unwrap();

        let competition = find_competition_by_id(&pool, 1).await.unwrap().unwrap();
        assert_eq!(competition.auction.tip_slot, 75);
        assert_eq!(competition.auction.deadline_slot, 100);
        assert_eq!(competition.auction.order_uids, vec![ByteArray(uid)]);
        assert_eq!(
            competition.auction.price_values,
            vec![BigDecimal::from(5), BigDecimal::from(7)]
        );
        assert_eq!(competition.tx_signatures, vec![ByteArray([9u8; 64])]);
        let solutions: Vec<_> = competition
            .solutions
            .iter()
            .map(|solution| {
                (
                    solution.ranking,
                    solution.solver.0[0],
                    solution.is_winner,
                    solution.filtered_out,
                    solution.reference_score.clone(),
                    solution.tx_signature,
                )
            })
            .collect();
        assert_eq!(
            solutions,
            vec![
                (2, 0xA2, false, true, None, None),
                (
                    1,
                    0xA1,
                    true,
                    false,
                    Some(BigDecimal::from(4)),
                    Some(ByteArray([9u8; 64]))
                ),
            ]
        );
        let trades: Vec<_> = competition
            .trades
            .iter()
            .map(|trade| {
                (
                    trade.solution_uid,
                    trade.executed_sell.to_string(),
                    trade.sell_token.0[0],
                    trade.buy_token.0[0],
                )
            })
            .collect();
        assert_eq!(
            trades,
            vec![
                (0, "90".to_owned(), 0xAA, 0xAB),
                (1, "50".to_owned(), 0xAA, 0xAB)
            ]
        );

        // The attributed settlement resolves its auction, an unattributed or
        // unknown signature nothing.
        let by_signature = find_competition_by_tx_signature(&pool, [9u8; 64])
            .await
            .unwrap()
            .unwrap();
        assert_eq!(by_signature.auction.id, 1);
        for unresolved in [[7u8; 64], [8u8; 64]] {
            assert!(
                find_competition_by_tx_signature(&pool, unresolved)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        // Auction 2 stays hidden until the indexer passes its deadline, so
        // auction 1 is the latest. The unfiltered read serves it already.
        assert!(find_competition_by_id(&pool, 2).await.unwrap().is_none());
        let unfiltered = find_competition_by_id_unfiltered(&pool, 2)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(unfiltered.auction.id, 2);
        assert!(
            find_competition_by_id_unfiltered(&pool, 3)
                .await
                .unwrap()
                .is_none()
        );
        let latest = find_latest_competition(&pool).await.unwrap().unwrap();
        assert_eq!(latest.auction.id, 1);
        sqlx::query("UPDATE solana.indexer_state SET slot = 200")
            .execute(&pool)
            .await
            .unwrap();
        let latest = find_latest_competition(&pool).await.unwrap().unwrap();
        assert_eq!(latest.auction.id, 2);
        assert!(find_competition_by_id(&pool, 3).await.unwrap().is_none());
    }

    /// The latest trade's settlement picks the auction, else the latest
    /// auction a solution executed the order in. Solutions list worst to
    /// best, a solution without the order carries no amounts.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_order_solutions() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;
        sqlx::query(
            "TRUNCATE solana.competition_auctions, solana.proposed_solutions, \
             solana.proposed_trade_executions",
        )
        .execute(&pool)
        .await
        .unwrap();
        for auction in [1i64, 2] {
            sqlx::query(
                "INSERT INTO solana.competition_auctions (id, tip_slot, deadline_slot, \
                 order_uids, price_tokens, price_values) VALUES ($1, 1, 100, '{}', '{}', '{}')",
            )
            .bind(auction)
            .execute(&pool)
            .await
            .unwrap();
        }
        // (auction, solution uid, solver, winner, score, executes the order)
        for (auction, solution, solver, winner, score, executes) in [
            (1i64, 0i64, 0xA1u8, true, 9i64, true),
            (2, 0, 0xB1, true, 8, true),
            (2, 1, 0xB2, true, 7, false),
            (2, 2, 0xB3, false, 5, true),
        ] {
            sqlx::query(
                "INSERT INTO solana.proposed_solutions (auction_id, uid, id, solver, is_winner, \
                 filtered_out, score) VALUES ($1, $2, $2, $3, $4, false, $5)",
            )
            .bind(auction)
            .bind(solution)
            .bind(ByteArray([solver; 32]))
            .bind(winner)
            .bind(score)
            .execute(&pool)
            .await
            .unwrap();
            if executes {
                sqlx::query(
                    "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, \
                     order_uid, executed_sell, executed_buy) VALUES ($1, $2, $3, $4, $5)",
                )
                .bind(auction)
                .bind(solution)
                .bind(ByteArray(uid))
                .bind(BigDecimal::from(score * 10))
                .bind(BigDecimal::from(score * 20))
                .execute(&pool)
                .await
                .unwrap();
            }
        }
        let view = |rows: Vec<SolutionRow>| {
            rows.into_iter()
                .map(|row| (row.solver.0[0], row.executed_buy.map(|buy| buy.to_string())))
                .collect::<Vec<_>>()
        };

        let second_auction = [
            (0xB3, Some("100".to_owned())),
            (0xB2, None),
            (0xB1, Some("160".to_owned())),
        ];
        let latest = find_order_solutions(&pool, uid).await.unwrap();
        assert_eq!(view(latest), second_auction);

        // A trade pins the auction that settled it, the latest trade wins.
        for (slot, auction, signature) in [(42i64, 1i64, 9u8), (43, 2, 8)] {
            sqlx::query(
                "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, \
                 auction_id) VALUES ($1, $2, 1, $3, $4)",
            )
            .bind(slot)
            .bind(ByteArray([signature; 64]))
            .bind(ByteArray([0xA1; 32]))
            .bind(auction)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO solana.trades (tx_signature, instruction_index, order_uid, \
                 sell_amount, buy_amount, fee_amount, slot) VALUES ($1, 1, $2, 400, 200, 0, $3)",
            )
            .bind(ByteArray([signature; 64]))
            .bind(ByteArray(uid))
            .bind(slot)
            .execute(&pool)
            .await
            .unwrap();
            let settled = find_order_solutions(&pool, uid).await.unwrap();
            let expected = match auction {
                1 => vec![(0xA1, Some("180".to_owned()))],
                _ => second_auction.to_vec(),
            };
            assert_eq!(view(settled), expected);
        }

        assert!(
            find_order_solutions(&pool, [0x99; 32])
                .await
                .unwrap()
                .is_empty()
        );
    }
}
