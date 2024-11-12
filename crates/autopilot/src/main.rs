shared::use_custom_global_allocator!();

#[tokio::main]
async fn main() {
    autopilot::start(std::env::args()).await;
}
