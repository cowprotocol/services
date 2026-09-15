//! Persistence helpers for quote competitions. Shared between the orderbook
//! and autopilot `QuoteStoring::save` implementations so both flows land the
//! same rows in the DB.

use {
    crate::{event_storing_helpers::create_quote_row, order_quoting::QuoteCompetition},
    alloy::primitives::{Address, U256},
    anyhow::{Context, Result},
    database::{PgTransaction, auction::AuctionId, fast_path},
    model::{order::OrderKind, quote::QuoteId},
    price_estimation::native::to_normalized_price,
    serde::{Deserialize, Serialize},
    std::collections::HashMap,
};

/// All data necessary to eventually convert the quote competition into a
/// regular auction which is used for the fast path feature.
///
/// This implements `Serialize` and `Deserialize` because it gets stored
/// in the temporary `quote_competitions` table before the autopilot
/// eventually moves the data into the permanent tables when it initiates
/// the fast path execution of the associated order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StagedQuoteCompetition {
    /// Unique id generated from the usual auction_id counter.
    pub auction_id: AuctionId,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StagedSolution {
    pub solution_uid: usize,
    pub solver: Address,
    pub solution_id: u64,
    pub is_winner: bool,
    pub quoted_sell: U256,
    pub quoted_buy: U256,
}

/// Persists a quote row and — for fast-path quotes — stages the associated
/// competition data in `quote_competitions`.
pub async fn save_quote_competition(
    tx: &mut PgTransaction<'_>,
    data: QuoteCompetition,
) -> Result<QuoteId> {
    let row = create_quote_row(&data)?;
    let id = database::quotes::save(&mut *tx, &row).await?;

    if let Some(auction_id) = data.metadata.auction_id {
        stage_competition(tx, id, auction_id, &data).await?;
    }

    Ok(id)
}

async fn stage_competition(
    tx: &mut PgTransaction<'_>,
    quote_id: QuoteId,
    auction_id: AuctionId,
    data: &QuoteCompetition,
) -> Result<()> {
    // Bail out on responses that can never actually settle — the fast-path
    // handler needs each solution's `solution_id` to instruct the driver.
    let quotes = data.quotes();
    let Some(winner) = quotes.first() else {
        tracing::error!(auction_id, "fast path quote competition without any quotes");
        return Ok(());
    };
    if winner.solution_id.is_none() {
        tracing::error!(
            auction_id,
            solver = ?winner.solver,
            "winning quote is missing a solution id; skipping competition storage"
        );
        return Ok(());
    }

    let solutions = quotes
        .iter()
        .enumerate()
        .filter_map(|(index, quote)| {
            Some(StagedSolution {
                solver: quote.solver,
                solution_id: quote.solution_id?,
                solution_uid: index,
                is_winner: index == 0,
                quoted_sell: quote.quoted_sell_amount,
                quoted_buy: quote.quoted_buy_amount,
            })
        })
        .collect();

    let competition = StagedQuoteCompetition {
        auction_id,
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
