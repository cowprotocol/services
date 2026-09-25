//! Persistence helpers for quote competitions. Shared between the orderbook
//! and autopilot `QuoteStoring::save` implementations so both flows land the
//! same rows in the DB.

use {
    crate::{
        event_storing_helpers::{create_db_search_parameters, create_quote_row},
        order_quoting::{QuoteCompetition, QuoteData, QuoteSearchParameters},
    },
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    chrono::{DateTime, Utc},
    database::{PgPool, PgTransaction, fast_path},
    model::{order::OrderKind, quote::QuoteId},
    price_estimation::native::to_normalized_price,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// All data necessary to eventually convert the quote competition into a
/// regular auction which is used for the fast path feature.
///
/// This implements `Serialize` and `Deserialize` because it gets stored
/// in the temporary `quote_competitions` table (keyed by the quote id) before
/// the autopilot eventually moves the data into the permanent tables when it
/// initiates the fast path execution of the associated order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StagedQuoteCompetition {
    /// The sell/buy token pair the quote was requested for.
    pub sell_token: Address,
    pub buy_token: Address,
    /// Order side, stored so the promoted `orders` row matches the placed
    /// order.
    pub side: OrderKind,
    /// Native prices captured at quote time.
    pub native_prices: HashMap<Address, U256>,
    /// One entry per participating solver. Ordered from best to worst
    /// so the winning solver is always at index 0.
    pub solutions: Vec<StagedSolution>,
}

/// One solver's quote in a staged competition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StagedSolution {
    /// Zero-based rank of the solution, best first: the `proposed_solutions`
    /// uid the public ranking is derived from.
    pub solution_uid: usize,
    pub solver: Address,
    pub is_winner: bool,
    pub quoted_sell: U256,
    pub quoted_buy: U256,
    /// The quote's gas fee in the sell token. The fast path nets it out of the
    /// bid the same way the user-facing quote does. Defaults to 0 for rows
    /// staged before this field existed (no gas adjustment, prior behaviour).
    #[serde(default)]
    pub fee: U256,
    /// Id the solver was asked with; its driver cached the solution under it,
    /// which is what the fast path settles by, and the promoted solution row
    /// is identified by it.
    pub quote_id: QuoteId,
}

/// Stores a quote (and, for fast-path quotes, its staged competition data).
pub async fn save_quote(pool: &PgPool, data: QuoteCompetition) -> Result<QuoteId> {
    let mut tx = pool.begin().await?;
    let id = save_quote_competition(&mut tx, data).await?;
    tx.commit().await?;
    Ok(id)
}

/// Persists a quote row under the id its winning solver was asked with (see
/// `QuoteCompetitionMetadata::quote_id`) and, for fast-path quotes, stages the
/// associated competition in `quote_competitions`.
async fn save_quote_competition(
    tx: &mut PgTransaction<'_>,
    data: QuoteCompetition,
) -> Result<QuoteId> {
    let row = create_quote_row(&data)?;
    let id = database::quotes::save(&mut *tx, &row).await?;

    if data.metadata.fast_path {
        stage_competition(tx, id, &data).await?;
    }

    Ok(id)
}

/// Looks up a quote by id.
pub async fn get_quote(pool: &PgPool, id: QuoteId) -> Result<Option<QuoteData>> {
    let mut ex = pool.acquire().await?;
    let quote = database::quotes::get(&mut ex, id).await?;
    quote.map(TryFrom::try_from).transpose()
}

/// Finds the most recent quote matching the given search parameters.
pub async fn find_quote(
    pool: &PgPool,
    params: QuoteSearchParameters,
    expiration: DateTime<Utc>,
) -> Result<Option<(QuoteId, QuoteData)>> {
    let mut ex = pool.acquire().await?;
    let params = create_db_search_parameters(params, expiration);
    let quote = database::quotes::find(&mut ex, &params)
        .await
        .context("failed finding quote by parameters")?;
    quote
        .map(|quote| Ok((quote.id, quote.try_into()?)))
        .transpose()
}

async fn stage_competition(
    tx: &mut PgTransaction<'_>,
    quote_id: QuoteId,
    data: &QuoteCompetition,
) -> Result<()> {
    let quotes = data.quotes();
    // Only a quote a solver produced can be settled through the fast path: its
    // driver cached the solution under the quote id. Trivial quotes (ETH/WETH
    // wrapping) come from no solver and carry no id.
    if quotes
        .first()
        .is_none_or(|winner| winner.quote_id.is_none())
    {
        tracing::warn!(
            quote_id,
            "fast path quote competition without a solver-produced winner; not staged"
        );
        return Ok(());
    }

    let solutions = quotes
        .iter()
        .filter_map(|quote| quote.quote_id.map(|quote_id| (quote, quote_id)))
        .enumerate()
        .map(|(index, (quote, quote_id))| StagedSolution {
            solution_uid: index,
            solver: quote.solver,
            is_winner: index == 0,
            quoted_sell: quote.quoted_sell_amount,
            quoted_buy: quote.quoted_buy_amount,
            fee: crate::fee::FeeParameters {
                gas_amount: quote.gas_amount,
                gas_price: data.metadata.gas_price,
                sell_token_price: data.metadata.sell_token_price,
            }
            .fee(),
            quote_id,
        })
        .collect();

    let competition = StagedQuoteCompetition {
        sell_token: data.request.sell_token,
        buy_token: data.request.buy_token,
        side: data.request.kind,
        native_prices: build_native_prices(data),
        solutions,
    };
    let json = serde_json::to_value(&competition)
        .context("failed to serialize staged quote competition")?;
    fast_path::save_competition(&mut *tx, quote_id, json)
        .await
        .context("failed to insert quote_competitions row")?;
    Ok(())
}

fn build_native_prices(data: &QuoteCompetition) -> HashMap<Address, U256> {
    [
        (data.request.sell_token, data.metadata.sell_token_price),
        (data.request.buy_token, data.metadata.buy_token_price),
    ]
    .into_iter()
    .filter_map(|(token, price)| Some((token, to_normalized_price(price)?)))
    .collect()
}
