alloc::custom_global_allocator!();

#[tokio::main]
async fn main() {
    alerter::start(std::env::args()).await;
}
