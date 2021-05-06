use crate::amm_pair_provider::{AmmPairProvider, SushiswapPairProvider, UniswapPairProvider};
use crate::pool_fetching::{Pool, PoolFetcher, PoolFetching};
use crate::Web3;
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

pub struct PoolAggregator {
    pub pool_fetchers: Vec<PoolFetcher>,
}

impl PoolAggregator {
    pub async fn from_sources(sources: Vec<BaselineSources>, chain_id: u64, web3: Web3) -> Self {
        let mut pool_fetchers = vec![];
        for source in sources.clone() {
            let pair_provider: Arc<dyn AmmPairProvider>;
            match source {
                BaselineSources::Uniswap => {
                    pair_provider = Arc::new(UniswapPairProvider {
                        factory: contracts::UniswapV2Factory::deployed(&web3)
                            .await
                            .expect("couldn't load deployed uniswap router"),
                        chain_id,
                    });
                }
                BaselineSources::Sushiswap => {
                    pair_provider = Arc::new(SushiswapPairProvider {
                        factory: contracts::SushiswapV2Factory::deployed(&web3)
                            .await
                            .expect("couldn't load deployed sushiswap router"),
                    });
                }
            }
            pool_fetchers.push(PoolFetcher {
                pair_provider,
                web3: web3.clone(),
            })
        }
        tracing::info!("Built Pool Aggregator from sources: {:?}", sources);
        Self { pool_fetchers }
    }
}

#[async_trait::async_trait]
impl PoolFetching for PoolAggregator {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>) -> Vec<Pool> {
        let mut pools = vec![];
        for fetcher in self.pool_fetchers.iter() {
            pools.extend(fetcher.fetch(token_pairs.clone()).await);
        }
        pools
    }
}
