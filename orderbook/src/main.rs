mod api;
mod orderbook;

use crate::orderbook::OrderBook;
use std::{net::SocketAddr, sync::Arc};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let orderbook = Arc::new(OrderBook::default());
    let filter = api::handle_all_routes(orderbook);
    let address = SocketAddr::new([0, 0, 0, 0].into(), 8080);
    tracing::info!(%address, "serving order book");
    warp::serve(filter).bind(address).await;
    tracing::error!("warp exited");
}
