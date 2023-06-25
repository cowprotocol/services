#[tokio::main]
async fn main() {
    solver::start(std::env::args()).await;
}
