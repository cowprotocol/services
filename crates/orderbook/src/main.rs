use clap::Parser;

#[tokio::main]
async fn main() {
    let args = orderbook::arguments::Arguments::parse();
    shared::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    tracing::info!("running order book with validated arguments:\n{}", args);
    shared::exit_process_on_panic::set_panic_hook();
    global_metrics::setup_metrics_registry(Some("gp_v2_api".into()), None);
    orderbook::run::run(args).await;
}
