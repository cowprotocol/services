use crate::amm_pair_provider::{AmmPairProvider, SushiswapPairProvider, UniswapPairProvider};
use crate::pool_fetching::{Pool, PoolFetcher, PoolFetching};
use crate::Web3;
use anyhow::Result;
use ethcontract::BlockNumber;
use model::TokenPair;
use std::collections::HashSet;
use std::sync::Arc;
use structopt::clap::arg_enum;

arg_enum! {
    #[derive(Debug, Clone)]
    pub enum BaselineSources {
        Uniswap,
        Sushiswap,
    }
}

pub async fn pair_providers(
    sources: &[BaselineSources],
    chain_id: u64,
    web3: &Web3,
) -> Vec<Arc<dyn AmmPairProvider>> {
    let mut providers: Vec<Arc<dyn AmmPairProvider>> = Vec::new();
    for source in sources {
        providers.push(match source {
            BaselineSources::Uniswap => Arc::new(UniswapPairProvider {
                factory: contracts::UniswapV2Factory::deployed(web3)
                    .await
                    .expect("couldn't load deployed uniswap router"),
                chain_id,
            }),
            BaselineSources::Sushiswap => Arc::new(SushiswapPairProvider {
                factory: contracts::SushiswapV2Factory::deployed(web3)
                    .await
                    .expect("couldn't load deployed sushiswap router"),
            }),
        })
    }
    providers
}

pub struct PoolAggregator {
    pub pool_fetchers: Vec<PoolFetcher>,
}

impl PoolAggregator {
    pub async fn from_providers(pair_providers: &[Arc<dyn AmmPairProvider>], web3: &Web3) -> Self {
        let pool_fetchers = pair_providers
            .iter()
            .cloned()
            .map(|pair_provider| PoolFetcher {
                pair_provider,
                web3: web3.clone(),
            })
            .collect();
        Self { pool_fetchers }
    }
}

#[async_trait::async_trait]
impl PoolFetching for PoolAggregator {
    async fn fetch(
        &self,
        token_pairs: HashSet<TokenPair>,
        at_block: BlockNumber,
    ) -> Result<Vec<Pool>> {
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
