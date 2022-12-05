use crate::liquidity::Liquidity;
use anyhow::Result;
use model::TokenPair;
use shared::recent_block_cache::Block;

#[mockall::automock]
#[async_trait::async_trait]
pub trait LiquidityCollecting: Send + Sync {
    async fn get_liquidity(&self, pairs: &[TokenPair], at_block: Block) -> Result<Vec<Liquidity>>;
}

pub struct LiquidityCollector {
    pub liquidity_sources: Vec<Box<dyn LiquidityCollecting>>,
}

#[async_trait::async_trait]
impl LiquidityCollecting for LiquidityCollector {
    async fn get_liquidity(&self, pairs: &[TokenPair], at_block: Block) -> Result<Vec<Liquidity>> {
        let futures = self
            .liquidity_sources
            .iter()
            .map(|source| source.get_liquidity(pairs, at_block));
        let amms: Vec<_> = futures::future::join_all(futures)
            .await
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        tracing::debug!("got {} AMMs", amms.len());
        Ok(amms)
    }
}
