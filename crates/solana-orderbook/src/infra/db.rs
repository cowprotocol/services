//! Database access for the Solana orderbook.

use {
    anyhow::{Context, Result},
    bigdecimal::BigDecimal,
    chrono::{DateTime, Utc},
    database::{
        byte_array::ByteArray,
        solana::{OrderEventLabel, OrderKind},
    },
    sqlx::{PgExecutor, PgPool},
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

/// The auction snapshot a competition ran on.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct CompetitionAuctionRow {
    pub id: i64,
    pub tip_slot: i64,
    pub deadline_slot: i64,
    pub order_uids: Vec<ByteArray<32>>,
    pub price_tokens: Vec<ByteArray<32>>,
    pub price_values: Vec<BigDecimal>,
}

/// One proposed solution. Uids are assigned best first, so the ranking is
/// `uid + 1` as on EVM.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ProposedSolutionRow {
    pub uid: i64,
    pub solver: ByteArray<32>,
    pub is_winner: bool,
    pub filtered_out: bool,
    pub score: BigDecimal,
}

/// One order execution of a proposed solution, with the order's mints.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ProposedTradeRow {
    pub solution_uid: i64,
    pub order_uid: ByteArray<32>,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
    pub sell_token: ByteArray<32>,
    pub buy_token: ByteArray<32>,
}

/// A settlement that claims the auction. `solution_uid` stays `None` until
/// the outcome verifier attributes settlements to solutions.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct CompetitionSettlementRow {
    pub tx_signature: ByteArray<64>,
    pub solution_uid: Option<i64>,
}

/// A winning solver's reference score.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct ReferenceScoreRow {
    pub solver: ByteArray<32>,
    pub reference_score: BigDecimal,
}

/// Everything persisted about one competition.
#[derive(Clone, Debug)]
pub struct CompetitionRows {
    pub auction: CompetitionAuctionRow,
    pub solutions: Vec<ProposedSolutionRow>,
    pub trades: Vec<ProposedTradeRow>,
    pub settlements: Vec<CompetitionSettlementRow>,
    pub reference_scores: Vec<ReferenceScoreRow>,
}

/// Read one competition. `None` when the auction ran no competition, or its
/// deadline has not passed the current auction's tip slot yet: solutions
/// stay hidden while they can still be submitted, as on EVM. No current
/// auction hides every competition.
pub async fn load_competition(pool: &PgPool, auction_id: i64) -> Result<Option<CompetitionRows>> {
    const AUCTION: &str = r#"
SELECT id, tip_slot, deadline_slot, order_uids, price_tokens, price_values
FROM solana.competition_auctions
WHERE id = $1 AND deadline_slot < (SELECT tip_slot FROM solana.auctions)
    "#;
    let Some(auction) = sqlx::query_as::<_, CompetitionAuctionRow>(AUCTION)
        .bind(auction_id)
        .fetch_optional(pool)
        .await
        .context("read solana.competition_auctions")?
    else {
        return Ok(None);
    };

    const SOLUTIONS: &str = r#"
SELECT uid, solver, is_winner, filtered_out, score
FROM solana.proposed_solutions
WHERE auction_id = $1
    "#;
    let solutions = sqlx::query_as(SOLUTIONS)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.proposed_solutions")?;

    const TRADES: &str = r#"
SELECT pte.solution_uid, pte.order_uid, pte.executed_sell, pte.executed_buy,
       o.sell_token, o.buy_token
FROM solana.proposed_trade_executions pte
JOIN solana.orders o ON o.uid = pte.order_uid
WHERE pte.auction_id = $1
    "#;
    let trades = sqlx::query_as(TRADES)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.proposed_trade_executions")?;

    // One transaction can carry several settlements of the auction, one per
    // instruction.
    const SETTLEMENTS: &str = r#"
SELECT DISTINCT ON (tx_signature) tx_signature, solution_uid
FROM solana.settlements
WHERE auction_id = $1
ORDER BY tx_signature, solution_uid NULLS LAST
    "#;
    let settlements = sqlx::query_as(SETTLEMENTS)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.settlements")?;

    const REFERENCE_SCORES: &str = r#"
SELECT solver, reference_score
FROM solana.reference_scores
WHERE auction_id = $1
    "#;
    let reference_scores = sqlx::query_as(REFERENCE_SCORES)
        .bind(auction_id)
        .fetch_all(pool)
        .await
        .context("read solana.reference_scores")?;

    Ok(Some(CompetitionRows {
        auction,
        solutions,
        trades,
        settlements,
        reference_scores,
    }))
}

/// The auction a settlement transaction claims. `None` when no settlement
/// was indexed under the signature.
pub async fn find_settlement_auction(
    ex: impl PgExecutor<'_>,
    tx_signature: [u8; 64],
) -> Result<Option<i64>> {
    sqlx::query_scalar("SELECT auction_id FROM solana.settlements WHERE tx_signature = $1 LIMIT 1")
        .bind(ByteArray(tx_signature))
        .fetch_optional(ex)
        .await
        .context("read solana.settlements by signature")
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

    /// A competition reads back with its solutions, executions, settlement
    /// and reference scores once the current tip passes its deadline, and
    /// its settlement's signature finds it.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_reads_a_competition_after_its_deadline() {
        let pool = PgPool::connect("postgresql://").await.unwrap();
        let uid = [0x11; 32];
        seed(&pool, uid, false).await;
        sqlx::query(
            "TRUNCATE solana.auctions, solana.competition_auctions, solana.proposed_solutions, \
             solana.proposed_trade_executions, solana.reference_scores",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO solana.competition_auctions (id, tip_slot, deadline_slot, order_uids, \
             price_tokens, price_values) VALUES (7, 90, 100, ARRAY[$1], ARRAY[$2], ARRAY[5])",
        )
        .bind(ByteArray(uid))
        .bind(ByteArray([0xAB; 32]))
        .execute(&pool)
        .await
        .unwrap();
        for (solution_uid, is_winner) in [(0i64, true), (1, false)] {
            sqlx::query(
                "INSERT INTO solana.proposed_solutions (auction_id, uid, id, solver, is_winner, \
                 filtered_out, score) VALUES (7, $1, 1, $2, $3, false, 10)",
            )
            .bind(solution_uid)
            .bind(ByteArray([0xCC; 32]))
            .bind(is_winner)
            .execute(&pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO solana.proposed_trade_executions (auction_id, solution_uid, \
                 order_uid, executed_sell, executed_buy) VALUES (7, $1, $2, 400, 200)",
            )
            .bind(solution_uid)
            .bind(ByteArray(uid))
            .execute(&pool)
            .await
            .unwrap();
        }
        sqlx::query(
            "INSERT INTO solana.reference_scores (auction_id, solver, reference_score) VALUES (7, \
             $1, 3)",
        )
        .bind(ByteArray([0xCC; 32]))
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO solana.settlements (slot, tx_signature, instruction_index, solver, \
             auction_id) VALUES (101, $1, 0, $2, 7)",
        )
        .bind(ByteArray([9u8; 64]))
        .bind(ByteArray([0xCC; 32]))
        .execute(&pool)
        .await
        .unwrap();

        // No current auction: hidden.
        assert!(load_competition(&pool, 7).await.unwrap().is_none());
        // The current tip has not passed the deadline: hidden.
        sqlx::query("INSERT INTO solana.auctions (tip_slot, json) VALUES (100, '{}')")
            .execute(&pool)
            .await
            .unwrap();
        assert!(load_competition(&pool, 7).await.unwrap().is_none());

        sqlx::query("UPDATE solana.auctions SET tip_slot = 101")
            .execute(&pool)
            .await
            .unwrap();
        let competition = load_competition(&pool, 7).await.unwrap().unwrap();
        assert_eq!(competition.auction.deadline_slot, 100);
        assert_eq!(competition.auction.order_uids, vec![ByteArray(uid)]);
        assert_eq!(competition.solutions.len(), 2);
        assert_eq!(competition.trades.len(), 2);
        assert_eq!(competition.trades[0].sell_token, ByteArray([0xAA; 32]));
        assert_eq!(competition.settlements.len(), 1);
        assert_eq!(competition.settlements[0].solution_uid, None);
        assert_eq!(
            competition.reference_scores[0].reference_score,
            BigDecimal::from(3)
        );
        assert!(load_competition(&pool, 8).await.unwrap().is_none());

        assert_eq!(
            find_settlement_auction(&pool, [9u8; 64]).await.unwrap(),
            Some(7)
        );
        assert_eq!(
            find_settlement_auction(&pool, [8u8; 64]).await.unwrap(),
            None
        );
    }
}
