alloc::custom_global_allocator!();

#[tokio::main]
async fn main() {
    orderbook::start(std::env::args()).await;
}
