#[allow(dead_code)]
mod batcher;
#[allow(dead_code)]
mod driver;
#[allow(dead_code)]
mod encoding;
#[allow(dead_code)]
mod ethereum;
#[allow(dead_code)]
mod orderbook;
#[allow(dead_code)]
mod settlement;

#[tokio::main]
async fn main() {
    // TODO: create driver, call settle_if_needed every 10 seconds
    todo!("run driver")
}
