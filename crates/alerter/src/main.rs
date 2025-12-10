#[cfg(feature = "mimalloc-allocator")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(not(feature = "mimalloc-allocator"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    alerter::start(std::env::args()).await;
}
