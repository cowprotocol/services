#[tokio::main]
async fn main() {
    observe::panic_hook::install();

    // TODO implement Display for the arguments
    solvers::run::run(std::env::args(), None).await;
}
