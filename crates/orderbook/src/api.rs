mod cancel_order;
mod cancel_orders;
mod get_auction;
mod get_native_price;
mod get_order_by_uid;
mod get_orders_by_tx;
mod get_solver_competition;
mod get_trades;
mod get_user_orders;
mod post_order;
mod post_quote;
mod post_solver_competition;
mod replace_order;
mod version;

use {
    crate::{
        database::trades::TradeRetrieving,
        orderbook::Orderbook,
        solver_competition::SolverCompetitionStoring,
    },
    shared::{
        api::{error, finalize_router, ApiReply},
        order_quoting::QuoteHandler,
        price_estimation::native::NativePriceEstimating,
    },
    std::sync::Arc,
    warp::{Filter, Rejection, Reply},
};

pub fn handle_all_routes(
    database: Arc<dyn TradeRetrieving>,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    solver_competition: Arc<dyn SolverCompetitionStoring>,
    solver_competition_auth: Option<String>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let routes = vec![
        (
            "v1/create_order",
            post_order::post_order(orderbook.clone()).boxed(),
        ),
        (
            "v1/get_order",
            get_order_by_uid::get_order_by_uid(orderbook.clone()).boxed(),
        ),
        ("v1/get_trades", get_trades::get_trades(database).boxed()),
        (
            "v1/cancel_order",
            cancel_order::cancel_order(orderbook.clone()).boxed(),
        ),
        (
            "v1/cancel_orders",
            cancel_orders::filter(orderbook.clone()).boxed(),
        ),
        (
            "v1/replace_order",
            replace_order::filter(orderbook.clone()).boxed(),
        ),
        (
            "v1/get_user_orders",
            get_user_orders::get_user_orders(orderbook.clone()).boxed(),
        ),
        (
            "v1/get_orders_by_tx",
            get_orders_by_tx::get_orders_by_tx(orderbook.clone()).boxed(),
        ),
        ("v1/post_quote", post_quote::post_quote(quotes).boxed()),
        ("v1/auction", get_auction::get_auction(orderbook).boxed()),
        (
            "v1/solver_competition",
            get_solver_competition::get(solver_competition.clone()).boxed(),
        ),
        (
            "v1/solver_competition",
            post_solver_competition::post(solver_competition, solver_competition_auth).boxed(),
        ),
        ("v1/version", version::version().boxed()),
        (
            "v1/get_native_price",
            get_native_price::get_native_price(native_price_estimator).boxed(),
        ),
    ];

    finalize_router(routes, "orderbook::api::request_summary")
}
