use gperftools::heap_profiler::HEAP_PROFILER;

#[tokio::main]
async fn main() {
    HEAP_PROFILER
        .lock()
        .unwrap()
        .start("/Users/felixleupold/Gnosis/gp-v2-services/my-prof.hprof")
        .unwrap();
    driver::start(std::env::args()).await;
    HEAP_PROFILER.lock().unwrap().stop().unwrap();
}
