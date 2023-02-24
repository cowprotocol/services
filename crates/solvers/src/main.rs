#[tokio::main]
async fn main() {
    solvers::boundary::exit_process_on_panic::set_panic_hook();

    // TODO implement Display for the arguments
    solvers::run::run(std::env::args(), None).await;
}
