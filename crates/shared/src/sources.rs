//! Top-level module organizing all baseline liquidity sources.

pub mod balancer;
pub mod uniswap;

use self::uniswap::{
    pair_provider::{AmmPairProvider, SushiswapPairProvider, UniswapPairProvider},
    pool_fetching::{Pool, PoolFetching},
};
use crate::{recent_block_cache::Block, Web3};
use anyhow::Result;
use contracts::UniswapV2Factory;
use model::TokenPair;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use structopt::clap::arg_enum;

arg_enum! {
    #[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
    pub enum BaselineSource {
        Uniswap,
        Sushiswap,
        BalancerV2,
    }
}

pub async fn pair_providers(
    sources: &[BaselineSource],
    chain_id: u64,
    web3: &Web3,
    // Passed in explicitly to facilitate running with locally deployed contracts.
    uniswap: &UniswapV2Factory,
) -> HashMap<BaselineSource, Arc<dyn AmmPairProvider>> {
    let mut providers = HashMap::new();
    for source in sources.iter().copied() {
        let provider: Arc<dyn AmmPairProvider> = match source {
            BaselineSource::Uniswap => Arc::new(UniswapPairProvider {
                factory: uniswap.clone(),
                chain_id,
            }),
            BaselineSource::Sushiswap => Arc::new(SushiswapPairProvider {
                factory: contracts::SushiswapV2Factory::deployed(web3)
                    .await
                    .expect("couldn't load deployed sushiswap router"),
            }),
            BaselineSource::BalancerV2 => continue,
        };
        providers.insert(source, provider);
    }
    providers
}

pub struct PoolAggregator {
    pub pool_fetchers: Vec<Arc<dyn PoolFetching>>,
}

#[async_trait::async_trait]
impl PoolFetching for PoolAggregator {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        // vk: Using try join means if any pool fetcher fails we fail too. Alternatively we could
        // return the succeeding ones but I feel it is cleaner to forward the error.
        let results = futures::future::try_join_all(
            self.pool_fetchers
                .iter()
                .map(|pool_fetcher| pool_fetcher.fetch(token_pairs.clone(), at_block)),
        )
        .await?;
        Ok(results.into_iter().flatten().collect())
    }
}
