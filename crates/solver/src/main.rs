use clap::Parser;

#[tokio::main]
async fn main() {
    let args = solver::arguments::Arguments::parse();
    shared::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    shared::exit_process_on_panic::set_panic_hook();
    tracing::info!("running solver with validated arguments:\n{}", args);
    global_metrics::setup_metrics_registry(Some("gp_v2_solver".into()), None);
    solver::run::run(args).await;
}
