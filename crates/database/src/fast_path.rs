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
        orders::{BuyTokenDestination, OrderKind, RawInteraction, SellTokenSource, SigningScheme},
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

/// A fast-path order the autopilot handler still owes a `valid_from`
/// write to. `quote_id` / `competition` are populated when the order
/// went through the API quoter (staged competition available); ethflow
/// fast-path orders arrive without a staged competition and get `None`.
#[derive(Debug, sqlx::FromRow)]
pub struct PendingFastPathOrder {
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
    /// `quote_competitions.quote_id` — the staging row the handler must
    /// promote and then delete. `None` for orders without a staged
    /// competition (ethflow, missing quote).
    pub quote_id: Option<QuoteId>,
    /// `quote_competitions.competition` — the serialized
    /// `StagedQuoteCompetition` produced at quote time. The caller is
    /// responsible for `serde_json::from_value`-decoding it. `None` for
    /// orders without a staged competition.
    pub competition: Option<serde_json::Value>,
}

/// Returns the order iff it is a fast-path order whose `valid_from`
/// has not been populated yet — i.e. one the autopilot's fast-path
/// handler still needs to classify. Callers use `Some` as the "we own
/// this order" signal and branch further on
/// [`PendingFastPathOrder::competition`] to decide whether an
/// out-of-competition settle attempt is even possible.
#[instrument(skip_all)]
pub async fn pending_fast_path_order(
    ex: &mut PgConnection,
    uid: &OrderUid,
) -> Result<Option<PendingFastPathOrder>, sqlx::Error> {
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
        "qc.quote_id AS quote_id, qc.competition AS competition",
        " FROM orders o",
        " LEFT JOIN order_quotes oq ON oq.order_uid = o.uid",
        " LEFT JOIN quote_competitions qc ON qc.quote_id = oq.quote_id",
        " LEFT JOIN app_data ad ON ad.contract_app_data = o.app_data",
        " WHERE o.uid = $1 AND o.fast_path AND o.valid_from IS NULL",
        " LIMIT 1",
    );
    sqlx::query_as(QUERY).bind(uid).fetch_optional(ex).await
}
