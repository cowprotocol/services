mod cancel_order;
mod create_order;
mod get_auction;
mod get_fee_and_quote;
mod get_fee_info;
mod get_markets;
mod get_order_by_uid;
mod get_orders_by_tx;
mod get_solvable_orders;
mod get_solvable_orders_v2;
mod get_solver_competition;
mod get_trades;
mod get_user_orders;
mod post_quote;
pub mod post_solver_competition;
mod replace_order;

use crate::solver_competition::SolverCompetitionStoring;
use crate::{database::trades::TradeRetrieving, order_quoting::QuoteHandler, orderbook::Orderbook};
use shared::api::{error, finalize_router, internal_error, ApiReply};
use std::sync::Arc;
use warp::{Filter, Rejection, Reply};

pub fn handle_all_routes(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    solver_competition: Arc<dyn SolverCompetitionStoring>,
    solver_competition_auth: Option<String>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Routes for api v1.

    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let create_order = create_order::create_order(orderbook.clone())
        .map(|result| (result, "v1/create_order"))
        .boxed();
    let fee_info = get_fee_info::get_fee_info(quotes.clone())
        .map(|result| (result, "v1/fee_info"))
        .boxed();
    let get_order = get_order_by_uid::get_order_by_uid(orderbook.clone())
        .map(|result| (result, "v1/get_order"))
        .boxed();
    let get_solvable_orders = get_solvable_orders::get_solvable_orders(orderbook.clone())
        .map(|result| (result, "v1/get_solvable_orders"))
        .boxed();
    let get_trades = get_trades::get_trades(database)
        .map(|result| (result, "v1/get_trades"))
        .boxed();
    let cancel_order = cancel_order::cancel_order(orderbook.clone())
        .map(|result| (result, "v1/cancel_order"))
        .boxed();
    let replace_order = replace_order::filter(orderbook.clone())
        .map(|result| (result, "v1/replace_order"))
        .boxed();
    let get_amount_estimate = get_markets::get_amount_estimate(quotes.clone())
        .map(|result| (result, "v1/get_amount_estimate"))
        .boxed();
    let get_fee_and_quote_sell = get_fee_and_quote::get_fee_and_quote_sell(quotes.clone())
        .map(|result| (result, "v1/get_fee_and_quote_sell"))
        .boxed();
    let get_fee_and_quote_buy = get_fee_and_quote::get_fee_and_quote_buy(quotes.clone())
        .map(|result| (result, "v1/get_fee_and_quote_buy"))
        .boxed();
    let get_user_orders = get_user_orders::get_user_orders(orderbook.clone())
        .map(|result| (result, "v1/get_user_orders"))
        .boxed();
    let get_orders_by_tx = get_orders_by_tx::get_orders_by_tx(orderbook.clone())
        .map(|result| (result, "v1/get_orders_by_tx"))
        .boxed();
    let post_quote = post_quote::post_quote(quotes)
        .map(|result| (result, "v1/post_quote"))
        .boxed();
    let get_auction = get_auction::get_auction(orderbook.clone())
        .map(|result| (result, "v1/auction"))
        .boxed();
    let get_solver_competition = get_solver_competition::get(solver_competition.clone())
        .map(|result| (result, "v1/solver_competition"))
        .boxed();
    let post_solver_competition =
        post_solver_competition::post(solver_competition, solver_competition_auth)
            .map(|result| (result, "v1/solver_competition"))
            .boxed();

    let routes_v1 = warp::path!("api" / "v1" / ..)
        .and(
            create_order
                .or(fee_info)
                .unify()
                .or(get_order)
                .unify()
                .or(get_solvable_orders)
                .unify()
                .or(get_trades)
                .unify()
                .or(cancel_order)
                .unify()
                .or(replace_order)
                .unify()
                .or(get_amount_estimate)
                .unify()
                .or(get_fee_and_quote_sell)
                .unify()
                .or(get_fee_and_quote_buy)
                .unify()
                .or(get_user_orders)
                .unify()
                .or(get_orders_by_tx)
                .unify()
                .or(post_quote)
                .unify()
                .or(get_auction)
                .unify()
                .or(get_solver_competition)
                .unify()
                .or(post_solver_competition)
                .unify(),
        )
        .untuple_one()
        .boxed();

    // Routes for api v2.

    let get_solvable_orders_v2 = get_solvable_orders_v2::get_solvable_orders(orderbook)
        .map(|result| (result, "v2/get_solvable_orders"))
        .boxed();

    let routes_v2 = warp::path!("api" / "v2" / ..)
        .and(get_solvable_orders_v2)
        .untuple_one();

    // Routes combined

    let routes = routes_v1.or(routes_v2).unify().boxed();
    finalize_router(routes, "orderbook::api::request_summary")
}
