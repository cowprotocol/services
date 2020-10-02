mod api;
mod models;
use crate::api::run_api;
use crate::models::OrderBook;
use tokio::select;

#[tokio::main]
async fn main() {
    let orderbook = OrderBook::new();
    let handler_api = run_api(orderbook);
    select! {
        err = handler_api => {
            println!("run_api returned the following error {:?}", err);
        }
    }
}
