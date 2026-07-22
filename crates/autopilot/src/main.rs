#[cfg(feature = "mimalloc-allocator")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "mimalloc-allocator"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// dial9 telemetry configuration, read entirely from the environment because
/// the runtime must be built before the autopilot parses its own config.
///
/// Disabled unless `DIAL9_ENABLED=true`. When enabled, sealed trace segments
/// are uploaded to S3 for post-hoc analysis if `DIAL9_S3_BUCKET` is set. See
/// `docs/DIAL9.md` for the full set of `DIAL9_*` knobs.
fn dial9_config() -> dial9_tokio_telemetry::Dial9Config {
    dial9_tokio_telemetry::Dial9Config::from_env()
}

// dial9 wraps the Tokio runtime; it requires `--cfg tokio_unstable`, set for
// the whole workspace in `.cargo/config.toml`. Building without that cfg fails
// to compile (loudly) rather than silently shipping an autopilot without
// telemetry.
#[dial9_tokio_telemetry::main(config = dial9_config)]
async fn main() {
    // dial9's `TracedRuntime::block_on` requires a `Send` root future, so collect
    // the `!Send` `std::env::Args` before the first await instead of holding it.
    let args: Vec<String> = std::env::args().collect();
    autopilot::start(args.into_iter()).await;
}
