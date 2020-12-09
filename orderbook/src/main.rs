mod api;
mod orderbook;

use crate::orderbook::OrderBook;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::task;

const MAINTENANCE_INTERVAL: Duration = Duration::from_secs(10);

pub async fn orderbook_maintenance(orderbook: Arc<OrderBook>) -> ! {
    loop {
        tracing::debug!("running order book maintenance");
        orderbook.run_maintenance().await;
        tokio::time::delay_for(MAINTENANCE_INTERVAL).await;
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let orderbook = Arc::new(OrderBook::default());
    let filter = api::handle_all_routes(orderbook.clone());
    let address = SocketAddr::new([0, 0, 0, 0].into(), 8080);
    tracing::info!(%address, "serving order book");
    let serve_task = task::spawn(warp::serve(filter).bind(address));
    let maintenance_task = task::spawn(orderbook_maintenance(orderbook));
    tokio::select! {
        result = serve_task => tracing::error!(?result, "serve task exited"),
        result = maintenance_task => tracing::error!(?result, "maintenance task exited"),
    };
}
