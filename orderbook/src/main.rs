mod api;
mod models;

use crate::{api::run_api, models::OrderBook};
use tokio::select;

#[tokio::main]
async fn main() {
    let orderbook = OrderBook::default();
    let handler_api = run_api(orderbook.clone());
    select! {
        e = handler_api => {
            println!("run_api returned  {:?}", e);
        }
    };
}
