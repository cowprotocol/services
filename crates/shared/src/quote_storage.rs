//! Persistence helpers for quote competitions. Shared between the orderbook
//! and autopilot `QuoteStoring::save` implementations so both flows land the
//! same rows in the DB.

use {
    crate::{
        db_order_conversions::order_kind_into,
        event_storing_helpers::create_quote_row,
        order_quoting::{QuoteCompetition, QuoteMetadata, QuoteResponse},
    },
    alloy::primitives::U256,
    anyhow::{Context, Result},
    bigdecimal::{BigDecimal, Zero},
    database::{
        Address,
        OrderUid,
        PgTransaction,
        auction::{Auction, AuctionId},
        byte_array::ByteArray,
        solver_competition_v2::{
            self,
            Order as CompetitionOrder,
            QuoteJitOrder,
            Solution as CompetitionSolution,
        },
    },
    model::{
        DomainSeparator,
        order::{OrderData, OrderKind},
        quote::QuoteId,
        signature::Signature,
    },
    number::conversions::u256_to_big_decimal,
    price_estimation::{native::to_normalized_price, trade_finding::external::dto},
};

/// Persists a quote row and, when the competition carries an `auction_id`,
/// also populates the associated `competition_auctions`,
/// `proposed_solutions`, `proposed_trade_executions`, and `proposed_jit_orders`
/// tables.
///
/// Streaming quotes call this repeatedly for the same `auction_id`; any
/// prior rows for that id are deleted first so each call is idempotent and
/// the DB always reflects the latest competition.
///
/// `domain_separator` is required to recover the owner of each JIT order
/// and compute its `order_uid` before persistence.
///
/// The caller owns the transaction: this function performs no
/// `begin`/`commit` so callers can compose it with other statements.
pub async fn save_quote_competition(
    tx: &mut PgTransaction<'_>,
    data: QuoteCompetition,
    domain_separator: &DomainSeparator,
) -> Result<QuoteId> {
    let row = create_quote_row(&data)?;
    let id = database::quotes::save(&mut *tx, &row).await?;

    if let Some(auction_id) = data.metadata.auction_id {
        write_competition_tables(tx, auction_id, &data, domain_separator).await?;
    }

    Ok(id)
}

async fn write_competition_tables(
    tx: &mut PgTransaction<'_>,
    auction_id: AuctionId,
    data: &QuoteCompetition,
    domain_separator: &DomainSeparator,
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
    let mut jits: Vec<QuoteJitOrder> = Vec::new();
    for (index, quote) in data.quotes().iter().enumerate() {
        let Some(solution_id) = quote.solution_id else {
            continue;
        };
        let solution_uid = i64::try_from(index).expect("solution index fits in i64");

        // Placeholder for the user's future order — the real uid is written
        // when the order is placed. See `QuoteJitOrder` for why this row is
        // needed even though the user order isn't a JIT.
        let sell = u256_to_big_decimal(&quote.quoted_sell_amount);
        let buy = u256_to_big_decimal(&quote.quoted_buy_amount);
        let mut orders = vec![CompetitionOrder {
            uid: Default::default(),
            sell_token,
            buy_token,
            limit_sell: sell.clone(),
            limit_buy: buy.clone(),
            executed_sell: sell,
            executed_buy: buy,
            side,
        }];

        match encode_jit_orders(quote, solution_uid, domain_separator) {
            Ok(quote_jits) => {
                for (order, jit) in quote_jits {
                    orders.push(order);
                    jits.push(jit);
                }
            }
            Err(err) => {
                tracing::error!(
                    auction_id,
                    solver = ?quote.solver,
                    ?err,
                    "skipping solution: failed to encode JIT orders"
                );
                continue;
            }
        }

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
            // trade. JIT clearing prices aren't reflected here — the solver's
            // JIT execution amounts are stored directly on each JIT row.
            price_tokens: vec![sell_token, buy_token],
            price_values: vec![
                u256_to_big_decimal(&quote.quoted_buy_amount),
                u256_to_big_decimal(&quote.quoted_sell_amount),
            ],
        });
    }

    solver_competition_v2::save_from_quote(tx, auction_id, &solutions, &jits)
        .await
        .context("failed to save quote competition solutions")?;

    Ok(())
}

/// Builds the pair of DB rows (trade execution + JIT metadata) for each
/// JIT order carried in a solver's quote response. Recovers the owner
/// from the signature and derives the `order_uid` so the JIT can be
/// joined against the future settlement.
fn encode_jit_orders(
    quote: &QuoteResponse,
    solution_uid: i64,
    domain_separator: &DomainSeparator,
) -> Result<Vec<(CompetitionOrder, QuoteJitOrder)>> {
    let QuoteMetadata::V1(metadata) = &quote.metadata;
    metadata
        .jit_orders
        .iter()
        .map(|jit| build_jit_rows(jit, solution_uid, domain_separator))
        .collect()
}

fn build_jit_rows(
    jit: &dto::JitOrder,
    solution_uid: i64,
    domain_separator: &DomainSeparator,
) -> Result<(CompetitionOrder, QuoteJitOrder)> {
    let kind = match jit.side {
        dto::Side::Buy => OrderKind::Buy,
        dto::Side::Sell => OrderKind::Sell,
    };
    let order_data = OrderData {
        sell_token: jit.sell_token,
        buy_token: jit.buy_token,
        receiver: Some(jit.receiver),
        sell_amount: jit.sell_amount,
        buy_amount: jit.buy_amount,
        valid_to: jit.valid_to,
        app_data: jit.app_data,
        fee_amount: U256::ZERO,
        kind,
        partially_fillable: jit.partially_fillable,
        sell_token_balance: jit.sell_token_source,
        buy_token_balance: jit.buy_token_destination,
    };
    let signature = Signature::from_bytes(jit.signing_scheme, &jit.signature)
        .context("failed to parse JIT signature")?;
    let owner = signature
        .recover_owner(&jit.signature, domain_separator, &order_data.hash_struct())
        .context("failed to recover JIT order owner")?;
    let order_uid: OrderUid = ByteArray(order_data.uid(domain_separator, owner).0);

    let sell_token: Address = ByteArray(*jit.sell_token.0);
    let buy_token: Address = ByteArray(*jit.buy_token.0);
    let limit_sell = u256_to_big_decimal(&jit.sell_amount);
    let limit_buy = u256_to_big_decimal(&jit.buy_amount);
    let (executed_sell, executed_buy) = jit_executed_amounts(jit)?;
    let db_side = order_kind_into(kind);

    let order = CompetitionOrder {
        uid: order_uid,
        sell_token,
        buy_token,
        limit_sell: limit_sell.clone(),
        limit_buy: limit_buy.clone(),
        executed_sell,
        executed_buy,
        side: db_side,
    };
    let jit_row = QuoteJitOrder {
        solution_uid,
        order_uid,
        sell_token,
        buy_token,
        limit_sell,
        limit_buy,
        side: db_side,
    };
    Ok((order, jit_row))
}

/// Derives (executed_sell, executed_buy) from the JIT's `executed_amount`
/// and limit price. The counterparty amount is computed proportionally at
/// the limit price — quote-time simulation doesn't produce clearing prices
/// for JIT trades, so we use the solver's declared limit rate as the
/// filled rate.
fn jit_executed_amounts(jit: &dto::JitOrder) -> Result<(BigDecimal, BigDecimal)> {
    let (executed_sell, executed_buy) = match jit.side {
        dto::Side::Sell => {
            let executed_sell = jit.executed_amount;
            let executed_buy = executed_sell
                .checked_mul(jit.buy_amount)
                .and_then(|v| v.checked_div(jit.sell_amount))
                .context("JIT sell executed amount overflow or zero sell limit")?;
            (executed_sell, executed_buy)
        }
        dto::Side::Buy => {
            let executed_buy = jit.executed_amount;
            let executed_sell = executed_buy
                .checked_mul(jit.sell_amount)
                .and_then(|v| v.checked_div(jit.buy_amount))
                .context("JIT buy executed amount overflow or zero buy limit")?;
            (executed_sell, executed_buy)
        }
    };
    Ok((
        u256_to_big_decimal(&executed_sell),
        u256_to_big_decimal(&executed_buy),
    ))
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
