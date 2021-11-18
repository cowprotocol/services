//! Top-level module organizing all baseline liquidity sources.

pub mod balancer_v2;
pub mod baoswap;
pub mod honeyswap;
pub mod sushiswap;
pub mod uniswap_v2;

use self::uniswap_v2::{
    pair_provider::PairProvider,
    pool_fetching::{Pool, PoolFetching},
};
use crate::{recent_block_cache::Block, Web3};
use anyhow::{bail, Result};
use model::TokenPair;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use structopt::clap::arg_enum;

arg_enum! {
    #[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
    pub enum BaselineSource {
        UniswapV2,
        Honeyswap,
        SushiSwap,
        BalancerV2,
        Baoswap,
    }
}

pub fn defaults_for_chain(chain_id: u64) -> Result<Vec<BaselineSource>> {
    Ok(match chain_id {
        1 | 4 => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::BalancerV2,
        ],
        100 => vec![
            BaselineSource::Honeyswap,
            BaselineSource::SushiSwap,
            BaselineSource::Baoswap,
        ],
        _ => bail!("unsupported chain {:#x}", chain_id),
    })
}

/// Returns a mapping of baseline sources to their respective pair providers.
pub async fn pair_providers(
    web3: &Web3,
    sources: &[BaselineSource],
) -> Result<HashMap<BaselineSource, PairProvider>> {
    let mut providers = HashMap::new();
    for source in sources {
        let provider = match source {
            BaselineSource::UniswapV2 => uniswap_v2::get_pair_provider(web3).await?,
            BaselineSource::SushiSwap => sushiswap::get_pair_provider(web3).await?,
            BaselineSource::Honeyswap => honeyswap::get_pair_provider(web3).await?,
            BaselineSource::Baoswap => baoswap::get_pair_provider(web3).await?,
            BaselineSource::BalancerV2 => continue,
        };

        providers.insert(*source, provider);
    }
    Ok(providers)
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
