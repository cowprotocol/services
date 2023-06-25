#[tokio::main]
async fn main() {
    alerter::start(std::env::args()).await;
}
