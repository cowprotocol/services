#![allow(dead_code)]

mod batcher;
mod driver;
mod encoding;
mod ethereum;
mod naive_solver;
mod orderbook;
mod settlement;

#[tokio::main]
async fn main() {
    tracing_setup::initialize("WARN,solver=DEBUG");
    tracing::info!("starting solver");
    todo!("run driver")
}
