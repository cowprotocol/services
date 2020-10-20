pub mod filter;
pub mod handler;

use crate::models::OrderBook;
use filter::{get, post_order};
use std::future::Future;
use warp::Filter;

pub fn run_api(orderbook: OrderBook) -> impl Future<Output = ()> + 'static {
    let routes = post_order(orderbook.clone()).or(get(orderbook));
    warp::serve(routes).bind(([127, 0, 0, 1], 3030))
}
