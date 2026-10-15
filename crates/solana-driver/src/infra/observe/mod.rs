//! Observability for the Solana driver, built on the shared `observe` crate.

/// Set up the observability. The `obs_config` argument configures the tokio
/// tracing framework.
pub fn init(obs_config: observe::Config) {
    observe::panic_hook::install();
    observe::tracing::init::initialize_reentrant(&obs_config);
}
