#![allow(dead_code)]

mod batcher;
mod driver;
mod encoding;
mod ethereum;
mod naive_amm_settlement;
mod orderbook;
mod settlement;

#[tokio::main]
async fn main() {
    // TODO: create driver, call settle_if_needed every 10 seconds
    todo!("run driver")
}
