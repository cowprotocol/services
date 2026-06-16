#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

mod client;
mod config;
mod liquidity;
mod orderbook;
mod proposal;
mod run;
mod solver;

#[tokio::main]
async fn main() {
    run::start().await;
}
