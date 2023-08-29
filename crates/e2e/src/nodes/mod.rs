pub mod forked_node;
pub mod local_node;

pub const NODE_HOST: &str = "http://127.0.0.1:8545";

#[async_trait::async_trait(?Send)]
pub trait TestNode {
    async fn reset(&self);
}
