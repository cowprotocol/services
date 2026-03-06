#[tokio::main]
async fn main() {
    driver::start(std::env::args()).await;
}
