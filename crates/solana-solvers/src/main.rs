#[tokio::main]
async fn main() {
    solana_solvers::start(std::env::args()).await;
}
