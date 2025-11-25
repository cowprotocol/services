#[cfg(feature = "jemalloc-profiling")]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(feature = "jemalloc-profiling")]
#[unsafe(export_name = "malloc_conf")]
pub static MALLOC_CONF: &[u8] = b"prof:true,lg_prof_sample:23,prof_prefix:/tmp/jeprof.out\0";

#[cfg(not(feature = "jemalloc-profiling"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    solvers::start(std::env::args()).await;
}
