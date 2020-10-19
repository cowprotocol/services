mod api;
pub mod batcher;
mod models;
use crate::batcher::batch_process;
use crate::{api::run_api, models::OrderBook};
use std::time::Duration;
use tokio::{select, spawn, time::delay_for};

const SLEEP_DURATION_UNTIL_NEXT_SOLVING_ATTEMPT_IN_SEC: u64 = 1;

#[tokio::main]
async fn main() {
    let orderbook = OrderBook::default();
    let handler_api = run_api(orderbook.clone());
    tokio::spawn(run_driver(orderbook));
    select! {
        e = handler_api => {
            println!("run_api returned  {:?}", e);
        }
    };
}

async fn run_driver(orderbook: OrderBook) -> ! {
    loop {
        let orderbook_for_iteration = orderbook.clone();
        spawn(async move {
            let res = batch_process(orderbook_for_iteration)
                .await
                .map_err(|e| format!(" {:?} while async call batch_process", e));
            if let Err(e) = res {
                println!("{:}", e)
            };
        });
        delay_for(Duration::from_secs(
            SLEEP_DURATION_UNTIL_NEXT_SOLVING_ATTEMPT_IN_SEC,
        ))
        .await;
    }
}
