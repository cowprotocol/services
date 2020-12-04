mod filter;
mod handler;

use crate::orderbook::OrderBook;
use std::sync::Arc;
use warp::Filter;

pub fn handle_all_routes(
    orderbook: Arc<OrderBook>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    filter::create_order(orderbook.clone())
        .or(filter::get_orders(orderbook))
        .or(filter::get_fee_info())
}
