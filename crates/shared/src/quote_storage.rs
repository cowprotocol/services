//! Persistence helpers for quote competitions. Shared between the orderbook
//! and autopilot `QuoteStoring::save` implementations so both flows land the
//! same rows in the DB.

use {
    crate::{event_storing_helpers::create_quote_row, order_quoting::QuoteCompetition},
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    database::{PgTransaction, fast_path},
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
    pub solver: Address,
    pub is_winner: bool,
    pub quoted_sell: U256,
    pub quoted_buy: U256,
    /// Id the solver was asked with; its driver cached the solution under it,
    /// which is what the fast path settles by, and the promoted solution row
    /// is identified by it.
    pub quote_id: QuoteId,
}

/// Persists a quote row under the id its winning solver was asked with (see
/// `QuoteCompetitionMetadata::quote_id`) and, for fast-path quotes, stages the
/// associated competition in `quote_competitions`.
pub async fn save_quote_competition(
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
            solver: quote.solver,
            is_winner: index == 0,
            quoted_sell: quote.quoted_sell_amount,
            quoted_buy: quote.quoted_buy_amount,
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
