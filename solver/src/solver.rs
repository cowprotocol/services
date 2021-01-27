use crate::{liquidity::Liquidity, settlement::Settlement};
use anyhow::Result;

#[async_trait::async_trait]
pub trait Solver {
    async fn solve(&self, orders: Vec<Liquidity>) -> Result<Option<Settlement>>;
}
