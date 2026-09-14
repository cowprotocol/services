//! The quote route: what one order would trade for right now.

pub mod dto;

use {
    crate::{
        domain::{
            auction::{self, Auction, Order},
            order_uid::OrderUid,
            slot::Slot,
        },
        infra::api::{
            LoggingJson,
            State,
            error::{Error as ApiError, Kind},
        },
    },
    axum::{Json, http::StatusCode},
    solana_sdk::pubkey::Pubkey,
    tracing::Instrument,
};

/// Handle `POST /quote`: solve a one-order auction and answer with the
/// executed amounts.
pub(crate) async fn quote(
    state: axum::extract::State<State>,
    LoggingJson(request): LoggingJson<dto::QuoteRequest>,
) -> Result<Json<dto::QuoteResponse>, (StatusCode, Json<ApiError>)> {
    if request.sell_token == request.buy_token {
        return Err(Kind::QuoteSameTokens.into());
    }
    let side = auction::Side::from(request.kind);
    let auction = quote_auction(&request, side);

    let solutions = state
        .competition()
        .compute_solutions(&auction)
        .instrument(tracing::info_span!(
            "/quote",
            solver = %state.competition().solver_name(),
        ))
        .await?;

    // A solution fills one order, so the best quote is the trade that gives the
    // most: the largest buy for a sell order, the smallest sell for a buy
    // order.
    let quoted = solutions
        .iter()
        .filter_map(|solution| Some((solution.solver, solution.trades.first()?)));
    let best = match side {
        auction::Side::Sell => quoted.max_by_key(|(_, trade)| trade.executed_buy),
        auction::Side::Buy => quoted.min_by_key(|(_, trade)| trade.executed_sell),
    };
    let Some((solver, trade)) = best else {
        return Err(Kind::QuotingFailed.into());
    };

    Ok(Json(dto::QuoteResponse {
        sell_amount: trade.executed_sell,
        buy_amount: trade.executed_buy,
        solver,
    }))
}

/// The single-order auction a quote is solved in. Everything beyond the pair
/// and the amount is a placeholder: a quoted solution is never settled.
fn quote_auction(request: &dto::QuoteRequest, side: auction::Side) -> Auction {
    let (sell_amount, buy_amount) = match side {
        // The counter amount is the quote's answer, so its limit stays
        // unconstrained: zero to buy for a sell order, max to sell for a
        // buy order.
        auction::Side::Sell => (request.amount, 0),
        auction::Side::Buy => (u64::MAX, request.amount),
    };
    Auction {
        id: None,
        orders: vec![Order {
            uid: OrderUid([0; 32]),
            owner: Pubkey::default(),
            sell_token: request.sell_token,
            buy_token: request.buy_token,
            sell_token_account: Pubkey::default(),
            buy_token_account: Pubkey::default(),
            sell_amount,
            buy_amount,
            valid_to: u32::MAX,
            side,
            partially_fillable: false,
            order_pda: Pubkey::default(),
            app_data: [0; 32],
        }],
        deadline_slot: Slot(0),
        deadline: request.deadline,
    }
}
