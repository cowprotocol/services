use shared::alloc::JemallocMemoryProfiler;

#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[tokio::main]
async fn main() {
    if let Some(profiler) = JemallocMemoryProfiler::new() {
        profiler.run();
    }

    autopilot::start(std::env::args()).await;
}
