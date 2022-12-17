use crate::liquidity::Liquidity;
use anyhow::Result;
use model::TokenPair;
use shared::{baseline_solver::BaseTokens, recent_block_cache::Block};
use std::{collections::HashSet, sync::Arc};

#[mockall::automock]
#[async_trait::async_trait]
pub trait LiquidityCollecting: Send + Sync {
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>>;
}

pub struct LiquidityCollector {
    pub liquidity_sources: Vec<Box<dyn LiquidityCollecting>>,
    pub base_tokens: Arc<BaseTokens>,
}

#[async_trait::async_trait]
impl LiquidityCollecting for LiquidityCollector {
    async fn get_liquidity(
        &self,
        pairs: HashSet<TokenPair>,
        at_block: Block,
    ) -> Result<Vec<Liquidity>> {
        let pairs = self.base_tokens.relevant_pairs(pairs.into_iter());
        let futures = self
            .liquidity_sources
            .iter()
            .map(|source| source.get_liquidity(pairs.clone(), at_block));
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
