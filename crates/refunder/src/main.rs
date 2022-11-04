use clap::Parser;

#[tokio::main]
async fn main() {
    let args = refunder::arguments::Arguments::parse();
    shared::tracing::initialize(
        "warn,refunder=debug,shared=debug",
        tracing::Level::ERROR.into(),
    );
    shared::exit_process_on_panic::set_panic_hook();
    tracing::info!("running refunder with validated arguments:\n{}", args);
    refunder::main(args).await;
}
