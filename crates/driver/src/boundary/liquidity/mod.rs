use {
    crate::{
        boundary,
        domain::{eth, liquidity},
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
        token_info::{CachedTokenInfoFetcher, TokenInfoFetcher},
    },
    solver::{
        liquidity::Liquidity,
        liquidity_collector::{LiquidityCollecting, LiquidityCollector},
    },
    std::{
        collections::HashSet,
        num::{NonZeroU64, NonZeroUsize},
        sync::Arc,
        time::Duration,
    },
};

pub mod balancer;
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
    /// Creates a new fetcher for the specified configuration.
    pub async fn new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self> {
        let blocks = current_block::Arguments {
            block_stream_poll_interval_seconds: BLOCK_POLL_INTERVAL,
            block_stream_retriever_strategy: BlockRetrieverStrategy::EthCall,
        };

        let web3 = boundary::web3(eth);
        let block_stream = blocks.stream(web3.clone()).await?;
        let block_retriever = blocks.retriever(web3.clone());

        let token_info_fetcher = Box::new(TokenInfoFetcher { web3: web3.clone() });
        let token_info_fetcher = Arc::new(CachedTokenInfoFetcher::new(token_info_fetcher));

        let uni_v3: Vec<_> = future::join_all(
            config
                .uniswap_v3
                .iter()
                .map(|config| uniswap::v3::collector(eth, block_retriever.clone(), config)),
        )
        .await
        .into_iter()
        .try_collect()?;

        let balancer_weighted: Vec<_> =
            future::join_all(config.balancer_weighted.iter().map(|config| {
                balancer::weighted::collector(
                    eth,
                    block_retriever.clone(),
                    block_stream.clone(),
                    token_info_fetcher.clone(),
                    config,
                )
            }))
            .await
            .into_iter()
            .try_collect()?;

        let uni_v2: Vec<_> = future::join_all(
            config
                .uniswap_v2
                .iter()
                .map(|config| uniswap::v2::collector(eth, &block_stream, config)),
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
            blocks: block_stream,
            inner: LiquidityCollector {
                liquidity_sources: [uni_v2, uni_v3, balancer_weighted]
                    .into_iter()
                    .flatten()
                    .collect(),
                base_tokens: Arc::new(base_tokens),
            },
        })
    }

    /// Fetches liquidity for the specified auction.
    pub async fn fetch(
        &self,
        pairs: &HashSet<liquidity::TokenPair>,
    ) -> Result<Vec<liquidity::Liquidity>> {
        let pairs = pairs
            .iter()
            .map(|pair| {
                let (a, b) = pair.get();
                TokenPair::new(a.into(), b.into()).expect("a != b")
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
            .filter_map(|(index, liquidity)| {
                let id = liquidity::Id(index);
                match liquidity {
                    Liquidity::ConstantProduct(pool) => Some(uniswap::v2::to_domain(id, pool)),
                    Liquidity::BalancerWeighted(pool) => {
                        Some(balancer::weighted::to_domain(id, pool))
                    }
                    Liquidity::BalancerStable(_) => unreachable!(),
                    Liquidity::LimitOrder(_) => unreachable!(),
                    Liquidity::Concentrated(pool) => uniswap::v3::to_domain(id, pool),
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
