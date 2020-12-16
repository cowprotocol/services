mod filter;
mod handler;

use crate::orderbook::OrderBook;
use std::sync::Arc;
use warp::Filter;

pub fn handle_all_routes(
    orderbook: Arc<OrderBook>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let order_creation = filter::create_order(orderbook.clone());
    let order_getter = filter::get_orders(orderbook.clone());
    let fee_info = filter::get_fee_info();
    let order_by_uid = filter::get_order_by_uid(orderbook);
    warp::path!("api" / "v1" / ..).and(
        order_creation
            .or(order_getter)
            .or(fee_info)
            .or(order_by_uid),
    )
}
