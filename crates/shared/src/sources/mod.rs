//! Top-level module organizing all baseline liquidity sources.

pub mod balancer_v2;
pub mod swapr;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v3_pair_provider;

use {
    self::uniswap_v2::pool_fetching::{Pool, PoolFetching},
    crate::recent_block_cache::Block,
    anyhow::Result,
    chain::Chain,
    model::TokenPair,
    std::{collections::HashSet, sync::Arc},
};

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum BaselineSource {
    None,
    UniswapV2,
    Honeyswap,
    SushiSwap,
    BalancerV2,
    Baoswap,
    Swapr,
    ZeroEx,
    UniswapV3,
    TestnetUniswapV2,
}

pub fn defaults_for_network(chain: &Chain) -> Vec<BaselineSource> {
    match chain {
        Chain::Mainnet => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::Swapr,
            BaselineSource::BalancerV2,
            BaselineSource::ZeroEx,
            BaselineSource::UniswapV3,
        ],
        Chain::Goerli => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::BalancerV2,
        ],
        Chain::Gnosis => vec![
            BaselineSource::Honeyswap,
            BaselineSource::SushiSwap,
            BaselineSource::Baoswap,
            BaselineSource::Swapr,
        ],
        Chain::ArbitrumOne => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::Swapr,
            BaselineSource::BalancerV2,
            BaselineSource::ZeroEx,
            BaselineSource::UniswapV3,
        ],
        Chain::Base => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::Swapr,
            BaselineSource::BalancerV2,
            BaselineSource::ZeroEx,
            BaselineSource::UniswapV3,
        ],
        Chain::Sepolia => vec![BaselineSource::TestnetUniswapV2],
    }
}

pub struct PoolAggregator {
    pub pool_fetchers: Vec<Arc<dyn PoolFetching>>,
}

#[async_trait::async_trait]
impl PoolFetching for PoolAggregator {
    async fn fetch(&self, token_pairs: HashSet<TokenPair>, at_block: Block) -> Result<Vec<Pool>> {
        // vk: Using try join means if any pool fetcher fails we fail too. Alternatively
        // we could return the succeeding ones but I feel it is cleaner to
        // forward the error.
        let results = futures::future::try_join_all(
            self.pool_fetchers
                .iter()
                .map(|pool_fetcher| pool_fetcher.fetch(token_pairs.clone(), at_block)),
        )
        .await?;
        Ok(results.into_iter().flatten().collect())
    }
}
