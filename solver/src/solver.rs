use crate::settlement::Settlement;
use anyhow::Result;
use model::order::Order;

#[async_trait::async_trait]
pub trait Solver {
    async fn solve(&self, orders: Vec<Order>) -> Result<Option<Settlement>>;
}
