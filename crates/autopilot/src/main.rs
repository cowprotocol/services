// Conditional global allocator selection based on environment variable
#[cfg(all(feature = "jemalloc", not(feature = "mimalloc-allocator")))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(all(feature = "mimalloc-allocator", not(feature = "jemalloc")))]
#[global_allocator]
static ALLOC: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    #[cfg(feature = "jemalloc")]
    tracing::info!("Using jemalloc allocator");

    #[cfg(feature = "mimalloc-allocator")]
    tracing::info!("Using mimalloc allocator");

    #[cfg(not(any(feature = "jemalloc", feature = "mimalloc-allocator")))]
    tracing::info!("Using system default allocator");

    autopilot::start(std::env::args()).await;
}
