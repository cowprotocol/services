use clap::Parser;

#[tokio::main]
async fn main() {
    let args = orderbook::arguments::Arguments::parse();
    observe::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    tracing::info!("running order book with validated arguments:\n{}", args);
    observe::panic_hook::install();
    observe::metrics::setup_registry(Some("gp_v2_api".into()), None);
    orderbook::run::run(args).await;
}
