use {
    crate::{
        boundary,
        domain::{competition::order, eth, liquidity},
        infra::{self, blockchain::Ethereum},
    },
    anyhow::Result,
    futures::future,
    itertools::Itertools,
    model::TokenPair,
    shared::{
        baseline_solver::BaseTokens,
        current_block::{self, BlockRetrieverStrategy, CurrentBlockStream},
        recent_block_cache::{self, CacheConfig},
    },
    solver::{
        liquidity::Liquidity,
        liquidity_collector::{LiquidityCollecting, LiquidityCollector},
    },
    std::{
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
        time::Duration,
    },
};

pub mod uniswap;

/// The default poll interval for the block stream updating task.
const BLOCK_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The default pool caching configuration to use.
fn cache_config() -> CacheConfig {
    CacheConfig {
        number_of_blocks_to_cache: NonZeroU64::new(10).unwrap(),
        number_of_entries_to_auto_update: NonZeroUsize::new(200).unwrap(),
        maximum_recent_block_age: 4,
        max_retries: 5,
        delay_between_retries: Duration::from_secs(1),
    }
}

pub struct Fetcher {
    blocks: CurrentBlockStream,
    inner: LiquidityCollector,
}

impl Fetcher {
    /// Creates a new fether for the specified configuration.
    pub async fn new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self> {
        let blocks = current_block::Arguments {
            block_stream_poll_interval_seconds: BLOCK_POLL_INTERVAL,
            block_stream_retriever_strategy: BlockRetrieverStrategy::EthCall,
        }
        .stream(boundary::web3(eth))
        .await?;

        let liquidity_sources = future::join_all(
            config
                .uniswap_v2
                .iter()
                .map(|config| async { uniswap::v2::collector(eth, &blocks, config).await }),
        )
        .await
        .into_iter()
        .try_collect()?;
        let base_tokens = BaseTokens::new(
            eth.contracts().weth().address(),
            &config
                .base_tokens
                .iter()
                .copied()
                .map(eth::H160::from)
                .collect::<Vec<_>>(),
        );

        Ok(Self {
            blocks,
            inner: LiquidityCollector {
                liquidity_sources,
                base_tokens: Arc::new(base_tokens),
            },
        })
    }

    /// Fetches liquidity for the specified auction.
    pub async fn fetch(&self, orders: &[order::Order]) -> Result<Vec<liquidity::Liquidity>> {
        let pairs = orders
            .iter()
            .filter_map(|order| match order.kind {
                order::Kind::Market | order::Kind::Limit { .. } => {
                    TokenPair::new(order.sell.token.into(), order.buy.token.into())
                }
                order::Kind::Liquidity => None,
            })
            .collect();
        let block_number = self.blocks.borrow().number;

        let liquidity = self
            .inner
            .get_liquidity(pairs, recent_block_cache::Block::Number(block_number))
            .await?;

        let liquidity = liquidity
            .into_iter()
            .enumerate()
            .map(|(index, liquidity)| {
                let id = liquidity::Id(index);
                match liquidity {
                    Liquidity::ConstantProduct(pool) => uniswap::v2::to_domain(id, pool),
                    Liquidity::BalancerWeighted(_) => unreachable!(),
                    Liquidity::BalancerStable(_) => unreachable!(),
                    Liquidity::LimitOrder(_) => unreachable!(),
                    Liquidity::Concentrated(_) => unreachable!(),
                }
            })
            .collect();
        Ok(liquidity)
    }
}

impl std::fmt::Debug for Fetcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Fetcher")
            .field("blocks", &self.blocks)
            .field("inner", &"LiquidityCollector")
            .finish()
    }
}
