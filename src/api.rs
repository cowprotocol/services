pub mod filter;
pub mod handler;
use crate::models::OrderBook;
use core::future::Future;
use filter::get;
use filter::post_order;
use warp::Filter;

pub fn run_api(orderbook: OrderBook) -> impl Future<Output = ()> + 'static {
    let routes = post_order(orderbook.clone()).or(get(orderbook));
    warp::serve(routes).bind(([127, 0, 0, 1], 3030))
}
