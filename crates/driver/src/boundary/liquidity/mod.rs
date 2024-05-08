use {
    crate::{
        boundary,
        domain::{eth, liquidity},
        infra::{self, blockchain::Ethereum},
    },
    anyhow::Result,
    ethrpc::current_block::CurrentBlockStream,
    futures::future,
    model::TokenPair,
    shared::{
        baseline_solver::BaseTokens,
        http_client::HttpClientFactory,
        recent_block_cache::{self, CacheConfig},
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
pub mod swapr;
pub mod uniswap;
pub mod zeroex;

/// The default pool caching configuration to use.
fn cache_config() -> CacheConfig {
    CacheConfig {
        number_of_blocks_to_cache: NonZeroU64::new(10).unwrap(),
        number_of_entries_to_auto_update: NonZeroUsize::new(1000).unwrap(),
        maximum_recent_block_age: 4,
        max_retries: 5,
        delay_between_retries: Duration::from_secs(1),
    }
}

/// The default HTTP client to use for liquidity fetching.
fn http_client() -> reqwest::Client {
    // TODO: Should we allow `reqwest::Client` configuration here?
    HttpClientFactory::default().create()
}

pub struct Fetcher {
    blocks: CurrentBlockStream,
    inner: LiquidityCollector,
    swapr_routers: HashSet<eth::ContractAddress>,
}

impl Fetcher {
    /// Creates a new fetcher for the specified configuration.
    pub async fn new(eth: &Ethereum, config: &infra::liquidity::Config) -> Result<Self> {
        let block_stream = eth.current_block();
        let block_retriever = Arc::new(boundary::web3(eth));

        let uni_v2: Vec<_> = future::try_join_all(
            config
                .uniswap_v2
                .iter()
                .map(|config| uniswap::v2::collector(eth, block_stream, config)),
        )
        .await?;

        let swapr_routers = config.swapr.iter().map(|config| config.router).collect();
        let swapr: Vec<_> = future::try_join_all(
            config
                .swapr
                .iter()
                .map(|config| swapr::collector(eth, block_stream, config)),
        )
        .await?;

        let bal_v2: Vec<_> = config
            .balancer_v2
            .iter()
            .map(|config| {
                balancer::v2::collector(eth, block_stream.clone(), block_retriever.clone(), config)
            })
            .collect();

        let uni_v3: Vec<_> = config
            .uniswap_v3
            .iter()
            .map(|config| uniswap::v3::collector(eth, block_retriever.clone(), config))
            .collect();

        let zeroex: Vec<_> = future::try_join_all(
            config
                .zeroex
                .as_ref()
                .map(|config| zeroex::collector(eth, block_stream.clone(), config))
                .into_iter()
                .collect::<Vec<_>>(),
        )
        .await?;

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
            blocks: block_stream.clone(),
            inner: LiquidityCollector {
                liquidity_sources: [uni_v2, swapr, bal_v2, uni_v3, zeroex]
                    .into_iter()
                    .flatten()
                    .collect(),
                base_tokens: Arc::new(base_tokens),
            },
            swapr_routers,
        })
    }

    /// Fetches liquidity for the specified auction.
    pub async fn fetch(
        &self,
        pairs: &HashSet<liquidity::TokenPair>,
        block: infra::liquidity::AtBlock,
    ) -> Result<Vec<liquidity::Liquidity>> {
        let pairs = pairs
            .iter()
            .map(|pair| {
                let (a, b) = pair.get();
                TokenPair::new(a.into(), b.into()).expect("a != b")
            })
            .collect();

        let block = match block {
            infra::liquidity::AtBlock::Recent => recent_block_cache::Block::Recent,
            infra::liquidity::AtBlock::Latest => {
                let block_number = self.blocks.borrow().number;
                recent_block_cache::Block::Number(block_number)
            }
        };
        let liquidity = self.inner.get_liquidity(pairs, block).await?;

        let liquidity = liquidity
            .into_iter()
            .enumerate()
            .filter_map(|(index, liquidity)| {
                let id = liquidity::Id(index);
                match liquidity {
                    Liquidity::ConstantProduct(pool) => {
                        if self.swapr_routers.contains(&uniswap::v2::router(&pool)) {
                            swapr::to_domain(id, pool)
                        } else {
                            uniswap::v2::to_domain(id, pool)
                        }
                    }
                    Liquidity::BalancerWeighted(pool) => balancer::v2::weighted::to_domain(id, pool),
                    Liquidity::BalancerStable(pool) => balancer::v2::stable::to_domain(id, pool),
                    Liquidity::LimitOrder(pool) => zeroex::to_domain(id, pool),
                    Liquidity::Concentrated(pool) => uniswap::v3::to_domain(id, pool),
                }
                // Ignore "bad" liquidity - this allows the driver to continue
                // solving with the other good stuff.
                .ok()
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
