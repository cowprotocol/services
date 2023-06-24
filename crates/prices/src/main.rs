#[tokio::main]
async fn main() {
    prices::main(std::env::args()).await;
}
