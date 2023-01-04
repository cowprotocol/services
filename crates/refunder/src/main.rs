use clap::Parser;

#[tokio::main]
async fn main() {
    let args = refunder::arguments::Arguments::parse();
    shared::tracing::initialize(
        args.logging.log_filter.as_str(),
        args.logging.log_stderr_threshold,
    );
    shared::exit_process_on_panic::set_panic_hook();
    tracing::info!("running refunder with validated arguments:\n{}", args);
    global_metrics::setup_metrics_registry(Some("refunder".into()), None);
    refunder::main(args).await;
}
