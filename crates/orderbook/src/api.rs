mod cancel_order;
mod cancel_orders;
mod get_app_data;
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
    crate::{database::Postgres, orderbook::Orderbook},
    shared::{
        api::{box_filter, error, finalize_router, ApiReply},
        order_quoting::QuoteHandler,
        price_estimation::native::NativePriceEstimating,
    },
    std::sync::Arc,
    warp::{Filter, Rejection, Reply},
};

pub fn handle_all_routes(
    database: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    solver_competition_auth: Option<String>,
    native_price_estimator: Arc<dyn NativePriceEstimating>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    // Note that we add a string with endpoint's name to all responses.
    // This string will be used later to report metrics.
    // It is not used to form the actual server response.

    let routes = vec![
        (
            "v1/create_order",
            box_filter(post_order::post_order(orderbook.clone())),
        ),
        (
            "v1/get_order",
            box_filter(get_order_by_uid::get_order_by_uid(orderbook.clone())),
        ),
        (
            "v1/get_trades",
            box_filter(get_trades::get_trades(database.clone())),
        ),
        (
            "v1/cancel_order",
            box_filter(cancel_order::cancel_order(orderbook.clone())),
        ),
        (
            "v1/cancel_orders",
            box_filter(cancel_orders::filter(orderbook.clone())),
        ),
        (
            "v1/replace_order",
            box_filter(replace_order::filter(orderbook.clone())),
        ),
        (
            "v1/get_user_orders",
            box_filter(get_user_orders::get_user_orders(orderbook.clone())),
        ),
        (
            "v1/get_orders_by_tx",
            box_filter(get_orders_by_tx::get_orders_by_tx(orderbook.clone())),
        ),
        ("v1/post_quote", box_filter(post_quote::post_quote(quotes))),
        (
            "v1/auction",
            box_filter(get_auction::get_auction(orderbook)),
        ),
        (
            "v1/solver_competition",
            box_filter(get_solver_competition::get(Arc::new(database.clone()))),
        ),
        (
            "v1/solver_competition",
            box_filter(post_solver_competition::post(
                Arc::new(database.clone()),
                solver_competition_auth,
            )),
        ),
        ("v1/version", box_filter(version::version())),
        (
            "v1/get_native_price",
            box_filter(get_native_price::get_native_price(native_price_estimator)),
        ),
        ("v1/get_app_data", get_app_data::get(database).boxed()),
    ];

    finalize_router(routes, "orderbook::api::request_summary")
}
