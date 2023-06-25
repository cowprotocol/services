#[tokio::main]
async fn main() {
    autopilot::start(std::env::args()).await;
}
