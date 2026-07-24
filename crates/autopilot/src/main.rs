#[cfg(feature = "mimalloc-allocator")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "mimalloc-allocator"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn dial9_config() -> dial9_tokio_telemetry::Dial9Config {
    dial9_tokio_telemetry::Dial9Config::from_env()
}

#[dial9_tokio_telemetry::main(config = dial9_config)]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    autopilot::start(args.into_iter()).await;
}
