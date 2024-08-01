use {
    crate::{app_data, database::Postgres, orderbook::Orderbook, quoter::QuoteHandler},
    shared::{
        api::{box_filter, error, finalize_router, ApiReply},
        price_estimation::native::NativePriceEstimating,
    },
    std::sync::Arc,
    warp::{Filter, Rejection, Reply},
};

mod cancel_order;
mod cancel_orders;
mod get_app_data;
mod get_auction;
mod get_native_price;
mod get_order_by_uid;
mod get_order_status;
mod get_orders_by_tx;
mod get_solver_competition;
mod get_total_surplus;
mod get_trades;
mod get_user_orders;
mod post_order;
mod post_quote;
mod put_app_data;
mod version;

pub fn handle_all_routes(
    database: Postgres,
    orderbook: Arc<Orderbook>,
    quotes: Arc<QuoteHandler>,
    app_data: Arc<app_data::Registry>,
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
            "v1/get_order_status",
            box_filter(get_order_status::get_status(orderbook.clone())),
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
            box_filter(get_auction::get_auction(orderbook.clone())),
        ),
        (
            "v1/solver_competition",
            box_filter(get_solver_competition::get(Arc::new(database.clone()))),
        ),
        (
            "v1/solver_competition/latest",
            box_filter(get_solver_competition::get_latest(Arc::new(
                database.clone(),
            ))),
        ),
        ("v1/version", box_filter(version::version())),
        (
            "v1/get_native_price",
            box_filter(get_native_price::get_native_price(native_price_estimator)),
        ),
        (
            "v1/get_app_data",
            get_app_data::get(database.clone()).boxed(),
        ),
        (
            "v1/put_app_data",
            box_filter(put_app_data::filter(app_data)),
        ),
        (
            "v1/get_total_surplus",
            box_filter(get_total_surplus::get(database)),
        ),
    ];

    finalize_router(routes, "orderbook::api::request_summary")
}
