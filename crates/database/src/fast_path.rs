//! Database queries for the fast-path settlement feature.
//!
//! Fast-path orders reuse a quote's synthetic solver competition as the
//! actual settlement. This module owns:
//!
//! - the query that recovers everything the autopilot needs to fire the
//!   `/settle` call once a fast-path order is placed
//!   ([`single_fast_path_order`]),
//! - the promotion step that patches the placeholder rows written at quote time
//!   to reference the real `order_uid` ([`finalize_quote_competition`]),
//! - fetching every bid on the fast-path order ([`fast_path_bids`]) and the
//!   bulk update that stamps fee-adjusted `executed_sell`/`executed_buy` on
//!   each of them ([`apply_fees_to_fast_path_bids`]).

use {
    crate::{
        Address,
        AppId,
        OrderUid,
        PgTransaction,
        auction::AuctionId,
        orders::{BuyTokenDestination, OrderKind, RawInteraction, SellTokenSource, SigningScheme},
    },
    sqlx::{
        PgConnection,
        QueryBuilder,
        types::{
            BigDecimal,
            chrono::{DateTime, Utc},
        },
    },
    std::ops::DerefMut,
    tracing::instrument,
};

/// The columns needed to re-encode a fast-path settlement — the placed order
/// joined with its winning solution and recorded fill. Only what the driver
/// needs is selected; order metadata and quote data are left out.
#[derive(Debug, sqlx::FromRow)]
pub struct FastPathOrder {
    pub uid: OrderUid,
    pub owner: Address,
    pub creation_timestamp: DateTime<Utc>,
    pub sell_token: Address,
    pub buy_token: Address,
    pub sell_amount: BigDecimal,
    pub buy_amount: BigDecimal,
    pub valid_to: i64,
    pub app_data: AppId,
    pub kind: OrderKind,
    pub partially_fillable: bool,
    pub signature: Vec<u8>,
    pub receiver: Option<Address>,
    pub signing_scheme: SigningScheme,
    pub sell_token_balance: SellTokenSource,
    pub buy_token_balance: BuyTokenDestination,
    pub pre_interactions: Vec<RawInteraction>,
    pub post_interactions: Vec<RawInteraction>,
    /// Contents of the order's `app_data` document (from the `app_data`
    /// table). `None` when the full document was never uploaded.
    pub full_app_data: Option<Vec<u8>>,
    pub auction_id: AuctionId,
    pub solution_id: BigDecimal,
    pub solution_uid: i64,
    pub solver: Address,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
    /// The quote auction's native prices (token, normalized price).
    pub price_tokens: Vec<Address>,
    pub price_values: Vec<BigDecimal>,
}

/// Recovers what's needed to settle `uid` out of competition in one query, or
/// `None` when it is not a fast-path order (no persisted quote competition).
#[instrument(skip_all)]
pub async fn single_fast_path_order(
    ex: &mut PgConnection,
    uid: &OrderUid,
) -> Result<Option<FastPathOrder>, sqlx::Error> {
    #[rustfmt::skip]
    const QUERY: &str = const_format::concatcp!(
        "SELECT ",
        "o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, ",
        "o.sell_amount, o.buy_amount, o.valid_to, o.app_data, o.kind, ",
        "o.partially_fillable, o.signature, o.receiver, o.signing_scheme, ",
        "o.sell_token_balance, o.buy_token_balance, ",
        "array(SELECT (p.target, p.value, p.data) FROM interactions p",
        " WHERE p.order_uid = o.uid AND p.execution = 'pre' ORDER BY p.index) AS pre_interactions, ",
        "array(SELECT (p.target, p.value, p.data) FROM interactions p",
        " WHERE p.order_uid = o.uid AND p.execution = 'post' ORDER BY p.index) AS post_interactions, ",
        "ad.full_app_data AS full_app_data, ",
        "oq.auction_id AS auction_id, ps.id AS solution_id, ps.uid AS solution_uid, ps.solver AS solver, ",
        "pte.executed_sell AS executed_sell, pte.executed_buy AS executed_buy, ",
        "ca.price_tokens AS price_tokens, ca.price_values AS price_values",
        " FROM orders o",
        " JOIN order_quotes oq ON oq.order_uid = o.uid",
        " JOIN proposed_solutions ps ON ps.auction_id = oq.auction_id AND ps.is_winner",
        " JOIN proposed_trade_executions pte",
        " ON pte.auction_id = ps.auction_id AND pte.solution_uid = ps.uid AND pte.order_uid = o.uid",
        " JOIN competition_auctions ca ON ca.id = oq.auction_id",
        " LEFT JOIN app_data ad ON ad.contract_app_data = o.app_data",
        " WHERE o.uid = $1",
        " LIMIT 1",
    );
    sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
}

/// Because the final order uid is not known when we store the quote
/// competition data we use `0x000...000` as a sentinel value.
/// When an order gets placed referencing a quote competition this function
/// replaces the placeholder value with the now final order uid.
#[instrument(skip_all)]
pub async fn finalize_quote_competition(
    ex: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = r#"
WITH patch_te AS (
    UPDATE proposed_trade_executions
    SET order_uid = $1
    WHERE auction_id = $2 AND order_uid = $3
)
UPDATE competition_auctions
SET order_uids = ARRAY[$1]
WHERE id = $2
"#;
    sqlx::query(QUERY)
        .bind(order_uid)
        .bind(auction_id)
        .bind(OrderUid::default())
        .execute(ex.deref_mut())
        .await?;
    Ok(())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct FastPathBid {
    pub solution_uid: i64,
    pub executed_sell: BigDecimal,
    pub executed_buy: BigDecimal,
}

/// Returns every proposed trade execution recorded against the fast-path
/// order — one row per competing solver. Rows for JIT orders (which use
/// different `order_uid`s) are naturally excluded.
#[instrument(skip_all)]
pub async fn fast_path_bids(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    order_uid: OrderUid,
) -> Result<Vec<FastPathBid>, sqlx::Error> {
    const QUERY: &str = r#"
SELECT solution_uid, executed_sell, executed_buy
FROM proposed_trade_executions
WHERE auction_id = $1 AND order_uid = $2
"#;
    sqlx::query_as(QUERY)
        .bind(auction_id)
        .bind(order_uid)
        .fetch_all(ex)
        .await
}

/// Overwrites the executed amounts on every competing solver's bid for a
/// fast-path order in a single query. Each row is matched by its own
/// `solution_uid`, so different bids can be updated to different values.
#[instrument(skip_all)]
pub async fn apply_fees_to_fast_path_bids(
    ex: &mut PgConnection,
    auction_id: AuctionId,
    order_uid: OrderUid,
    bids: &[FastPathBid],
) -> Result<(), sqlx::Error> {
    if bids.is_empty() {
        return Ok(());
    }
    let mut query_builder = QueryBuilder::new(
        "UPDATE proposed_trade_executions AS pte SET executed_sell = v.executed_sell, \
         executed_buy = v.executed_buy FROM (",
    );
    query_builder.push_values(bids.iter(), |mut b, bid| {
        b.push_bind(bid.solution_uid)
            .push_bind(&bid.executed_sell)
            .push_bind(&bid.executed_buy);
    });
    query_builder.push(") AS v(solution_uid, executed_sell, executed_buy) WHERE pte.auction_id = ");
    query_builder.push_bind(auction_id);
    query_builder.push(" AND pte.order_uid = ");
    query_builder.push_bind(order_uid);
    query_builder.push(" AND pte.solution_uid = v.solution_uid");
    query_builder.build().execute(ex).await?;
    Ok(())
}
