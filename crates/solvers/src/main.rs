#[tokio::main]
async fn main() {
    solvers::start(std::env::args()).await;
}
