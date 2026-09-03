//! Persistence helpers for quote competitions. Shared between the orderbook
//! and autopilot `QuoteStoring::save` implementations so both flows land the
//! same rows in the DB.

use {
    crate::{
        db_order_conversions::order_kind_into,
        event_storing_helpers::create_quote_row,
        order_quoting::QuoteCompetition,
    },
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, Zero},
    database::{
        Address,
        PgTransaction,
        auction::{Auction, AuctionId},
        byte_array::ByteArray,
        solver_competition_v2::{self, Order as CompetitionOrder, Solution as CompetitionSolution},
    },
    model::quote::QuoteId,
    number::conversions::u256_to_big_decimal,
    price_estimation::native::to_normalized_price,
};

/// Persists a quote row and, when the competition carries an `auction_id`,
/// also populates the associated `competition_auctions`,
/// `proposed_solutions`, and `proposed_trade_executions` tables.
///
/// Streaming quotes call this repeatedly for the same `auction_id`; any
/// prior rows for that id are deleted first so each call is idempotent and
/// the DB always reflects the latest competition.
///
/// JIT orders proposed by solvers are intentionally *not* persisted here:
/// they can be recovered from the `quotes` table's `metadata` JSON blob if
/// needed, and the driver re-encodes them at settle time.
///
/// The caller owns the transaction: this function performs no
/// `begin`/`commit` so callers can compose it with other statements.
pub async fn save_quote_competition(
    tx: &mut PgTransaction<'_>,
    data: QuoteCompetition,
) -> Result<QuoteId> {
    let row = create_quote_row(&data)?;
    let id = database::quotes::save(&mut *tx, &row).await?;

    if let Some(auction_id) = data.metadata.auction_id {
        write_competition_tables(tx, auction_id, &data).await?;
    }

    Ok(id)
}

async fn write_competition_tables(
    tx: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    data: &QuoteCompetition,
) -> Result<()> {
    // Without a solution id we can't expect the solver to actually execute
    // this solution, so storing any competition rows would be misleading.
    let Some(winner) = data.quotes().first() else {
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

    solver_competition_v2::delete_by_auction_id(tx, auction_id)
        .await
        .context("failed to clear previous quote competition rows")?;

    let sell_token = ByteArray(*data.request.sell_token.0);
    let buy_token = ByteArray(*data.request.buy_token.0);
    let (native_price_tokens, native_price_values) = build_native_prices(data);

    let auction = Auction {
        id: auction_id,
        // Block, deadline, and order_uids are unknown at quote time; real
        // values are populated when the user places the order and a full
        // auction runs.
        block: 0,
        deadline: 0,
        order_uids: Vec::new(),
        price_tokens: native_price_tokens,
        price_values: native_price_values,
        surplus_capturing_jit_order_owners: Vec::new(),
        penalty_caps_native: Some(Vec::new()),
    };
    database::auction::save(&mut *tx, auction)
        .await
        .context("failed to save competition_auctions row")?;

    let side = order_kind_into(data.request.kind);
    let mut solutions: Vec<CompetitionSolution> = Vec::with_capacity(data.quotes().len());
    for (index, quote) in data.quotes().iter().enumerate() {
        let Some(solution_id) = quote.solution_id else {
            continue;
        };
        let solution_uid = i64::try_from(index).expect("solution index fits in i64");

        // Placeholder for the user's future order — the real uid is written
        // when the order is placed.
        let sell = u256_to_big_decimal(&quote.quoted_sell_amount);
        let buy = u256_to_big_decimal(&quote.quoted_buy_amount);
        let orders = vec![CompetitionOrder {
            uid: Default::default(),
            sell_token,
            buy_token,
            limit_sell: sell.clone(),
            limit_buy: buy.clone(),
            executed_sell: sell,
            executed_buy: buy,
            side,
        }];

        solutions.push(CompetitionSolution {
            uid: solution_uid,
            id: BigDecimal::from(solution_id),
            solver: ByteArray(*quote.solver.0),
            is_winner: index == 0,
            filtered_out: false,
            // No limit price exists at quote time, so surplus (and thus
            // score) is undefined; store 0 as a placeholder.
            score: BigDecimal::zero(),
            orders,
            // Natural single-trade UCP encoding for the user's placeholder
            // trade.
            price_tokens: vec![sell_token, buy_token],
            price_values: vec![
                u256_to_big_decimal(&quote.quoted_buy_amount),
                u256_to_big_decimal(&quote.quoted_sell_amount),
            ],
        });
    }

    solver_competition_v2::save_from_quote(tx, auction_id, &solutions)
        .await
        .context("failed to save quote competition solutions")?;

    Ok(())
}

fn build_native_prices(data: &QuoteCompetition) -> (Vec<Address>, Vec<BigDecimal>) {
    let mut tokens = Vec::with_capacity(2);
    let mut values = Vec::with_capacity(2);
    for (token, price) in [
        (data.request.sell_token, data.metadata.sell_token_price),
        (data.request.buy_token, data.metadata.buy_token_price),
    ] {
        if let Some(value) = to_normalized_price(price) {
            tokens.push(ByteArray(*token.0));
            values.push(u256_to_big_decimal(&value));
        }
    }
    (tokens, values)
}
