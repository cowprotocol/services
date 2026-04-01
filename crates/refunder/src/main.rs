#[cfg(all(unix, feature = "mimalloc-allocator"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(unix, not(feature = "mimalloc-allocator")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    refunder::start(std::env::args()).await
}
