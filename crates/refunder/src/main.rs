alloc::custom_global_allocator!();

#[tokio::main]
async fn main() {
    refunder::start(std::env::args()).await
}
