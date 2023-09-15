#[tokio::main]
async fn main() {
    refunder::start(std::env::args()).await
}
