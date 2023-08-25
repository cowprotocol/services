use clap::Parser;

#[tokio::main]
async fn main() {
    let args = refunder::arguments::Arguments::parse();
    observe::tracing::initialize(
        args.logging.log_filter.as_str(),
        args.logging.log_stderr_threshold,
    );
    observe::panic_hook::install();
    tracing::info!("running refunder with validated arguments:\n{}", args);
    observe::metrics::setup_registry(Some("refunder".into()), None);
    refunder::main(args).await;
}
