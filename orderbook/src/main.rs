mod api;
mod orderbook;

use crate::orderbook::OrderBook;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let orderbook = Arc::new(OrderBook::default());
    let filter = api::handle_all_routes(orderbook);
    let result = warp::serve(filter).bind(([127, 0, 0, 1], 8080)).await;
    println!("warp exited: {:?}", result);
}
