#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() {
    let commit = env!("VERGEN_GIT_DESCRIBE");
    //Log version at startup
    tracing::info!(%commit, "starting autopilot");

    autopilot::start(std::env::args()).await;
}
