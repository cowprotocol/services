use clap::Parser;

#[tokio::main]
async fn main() {
    let args = autopilot::arguments::Arguments::parse();
    observe::tracing::initialize(
        args.shared.logging.log_filter.as_str(),
        args.shared.logging.log_stderr_threshold,
    );
    observe::panic_hook::install();
    tracing::info!("running autopilot with validated arguments:\n{}", args);
    observe::metrics::setup_registry(Some("gp_v2_autopilot".into()), None);
    autopilot::main(args).await;
}
