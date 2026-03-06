#[tokio::main]
async fn main() {
    orderbook::start(std::env::args()).await;
}
