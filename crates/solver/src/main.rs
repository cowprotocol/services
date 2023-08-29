use clap::Parser;

#[tokio::main]
async fn main() {
    let args = solver::arguments::Arguments::parse();
    observe::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    observe::panic_hook::install();
    tracing::info!("running solver with validated arguments:\n{}", args);
    observe::metrics::setup_registry(Some("gp_v2_solver".into()), None);
    solver::run::run(args).await;
}
