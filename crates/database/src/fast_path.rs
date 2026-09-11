//! Database queries for the fast-path settlement feature.
//!
//! For fast path orders the data from the quote competition needs to be
//! persisted as if it was a regular auction. Because many quotes get requested
//! but never turned into an actual order the `quote_competitions` table only
//! keeps temporary data in the form of a JSON blob keyed by the `quotes.id`
//! which eventually gets "promoted" into the regular competition tables once
//! an order was actually placed and the fast path execution gets initiated.
//! We use a JSON blob because it offers the greated flexibility as the data
//! of a single `quote_competitions` row needs to be used to populate multiple
//! different tables (`competition_auctions`, `proposed_solutions`,
//! `proposed_trade_executions`, `reference_scores`, `fee_policies`).

use {
    crate::{
        Address,
        AppId,
        OrderUid,
        orders::{
            BuyTokenDestination,
            OrderClass,
            OrderKind,
            RawInteraction,
            SellTokenSource,
            SigningScheme,
        },
        quotes::QuoteId,
    },
    sqlx::{
        PgConnection,
        types::{
            BigDecimal,
            chrono::{DateTime, Utc},
        },
    },
    tracing::instrument,
};

/// Inserts one `quote_competitions` staging row for `quote_id`. Errors if a
/// row already exists — each stored quote gets exactly one competition. The
/// shape of the JSON payload is owned by the caller; this module doesn't
/// parse it.
#[instrument(skip_all)]
pub async fn save_competition(
    ex: &mut PgConnection,
    quote_id: QuoteId,
    competition: serde_json::Value,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "INSERT INTO quote_competitions (quote_id, competition) VALUES ($1, $2)";
    sqlx::query(QUERY)
        .bind(quote_id)
        .bind(competition)
        .execute(ex)
        .await?;
    Ok(())
}

/// Drops the `quote_competitions` staging row once its competition has been
/// promoted into the permanent tables.
#[instrument(skip_all)]
pub async fn delete_competition(
    ex: &mut PgConnection,
    quote_id: QuoteId,
) -> Result<(), sqlx::Error> {
    const QUERY: &str = "DELETE FROM quote_competitions WHERE quote_id = $1";
    sqlx::query(QUERY).bind(quote_id).execute(ex).await?;
    Ok(())
}

/// All data need to finalize the fast path processing and initiate the
/// settlement.
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
    /// The order's class (Market / Limit / Liquidity). Loaded here so
    /// `ProtocolFees::apply` can gate the protocol Volume policy on
    /// `OrderClass::Limit`.
    pub class: OrderClass,
    pub pre_interactions: Vec<RawInteraction>,
    pub post_interactions: Vec<RawInteraction>,
    /// Contents of the order's `app_data` document (from the `app_data`
    /// table). `None` when the full document was never uploaded.
    pub full_app_data: Option<Vec<u8>>,
    /// `quote_competitions.quote_id` — the staging row the handler must
    /// promote and then delete.
    pub quote_id: QuoteId,
    /// `quote_competitions.competition` — the serialized
    /// `StagedQuoteCompetition` produced at quote time. The caller is
    /// responsible for `serde_json::from_value`-decoding it.
    pub competition: serde_json::Value,
}

/// Recovers what's needed for the autopilot to finalize the fast path
/// data and intiate the settlement. Returns `None` if order is not a
/// fast path order or if it has already been handled.
#[instrument(skip_all)]
pub async fn unfinalized_fast_path_order(
    ex: &mut PgConnection,
    uid: &OrderUid,
) -> Result<Option<FastPathOrder>, sqlx::Error> {
    #[rustfmt::skip]
    const QUERY: &str = const_format::concatcp!(
        "SELECT ",
        "o.uid, o.owner, o.creation_timestamp, o.sell_token, o.buy_token, ",
        "o.sell_amount, o.buy_amount, o.valid_to, o.app_data, o.kind, ",
        "o.partially_fillable, o.signature, o.receiver, o.signing_scheme, ",
        "o.sell_token_balance, o.buy_token_balance, o.class, ",
        "array(SELECT (p.target, p.value, p.data) FROM interactions p",
        " WHERE p.order_uid = o.uid AND p.execution = 'pre' ORDER BY p.index) AS pre_interactions, ",
        "array(SELECT (p.target, p.value, p.data) FROM interactions p",
        " WHERE p.order_uid = o.uid AND p.execution = 'post' ORDER BY p.index) AS post_interactions, ",
        "ad.full_app_data AS full_app_data, ",
        "qc.quote_id AS quote_id, qc.competition AS competition",
        " FROM orders o",
        " JOIN order_quotes oq ON oq.order_uid = o.uid",
        " JOIN quote_competitions qc ON qc.quote_id = oq.quote_id",
        " LEFT JOIN app_data ad ON ad.contract_app_data = o.app_data",
        " WHERE o.uid = $1",
        " LIMIT 1",
    );
    sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
}
