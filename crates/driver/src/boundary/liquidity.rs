use {
    crate::{
        domain::{competition::auction, eth, liquidity},
        infra::blockchain::{contracts::ContractAt, Ethereum},
    },
    anyhow::Result,
    contracts::IUniswapLikeRouter,
    futures::future,
    itertools::Itertools,
    shared::{
        baseline_solver::BaseTokens,
        sources::uniswap_v2::{
            pair_provider::PairProvider as UniswapV2PairProvider,
            pool_fetching::{
                DefaultPoolReader as UniswapV2PoolReader,
                PoolFetcher as UniswapV2PoolFetcher,
                PoolReading,
            },
        },
    },
    solver::{
        liquidity::uniswap_v2::UniswapLikeLiquidity,
        liquidity_collector::{LiquidityCollecting, LiquidityCollector},
    },
    std::sync::Arc,
    web3::Web3,
};

pub struct Fetcher {
    inner: LiquidityCollector,
}

impl Fetcher {
    /// Creates a new fether for the specified configuration.
    pub async fn new(eth: &Ethereum, config: &liquidity::fetcher::Config) -> Result<Self> {
        let liquidity_sources = future::join_all(
            config
                .uniswap_v2
                .iter()
                .map(|config| async { uniswap_v2(eth, config).await }),
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
            inner: LiquidityCollector {
                liquidity_sources,
                base_tokens: Arc::new(base_tokens),
            },
        })
    }

    /// Fetches liquidity for the specified auction.
    pub async fn fetch(&self, _auction: &auction::Auction) -> Result<Vec<liquidity::Liquidity>> {
        todo!()
    }
}

impl std::fmt::Debug for Fetcher {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Fetcher")
            .field("inner", &"LiquidityCollector")
            .finish()
    }
}

async fn uniswap_v2(
    eth: &Ethereum,
    config: &liquidity::fetcher::config::UniswapV2,
) -> Result<Box<dyn LiquidityCollecting>> {
    let router = eth.contract_at::<IUniswapLikeRouter>(config.router);
    let settlement = eth.contracts().settlement().clone();
    let web3 = router.raw_instance().web3().clone();
    let pool_fether = {
        let factory = router.factory().call().await?;
        let pair_provider = UniswapV2PairProvider {
            factory,
            init_code_digest: config.pool_code.0,
        };
        let pool_reader = UniswapV2PoolReader::for_pair_provider(pair_provider, web3.clone());

        Arc::new(UniswapV2PoolFetcher {
            pool_reader,
            web3: web3.clone(),
        })
    };

    Ok(Box::new(UniswapLikeLiquidity::new(
        router,
        settlement,
        web3,
        pool_fether,
    )))
}

impl ContractAt for IUniswapLikeRouter {
    fn at(web3: &Web3<web3::transports::Http>, address: eth::ContractAddress) -> Self {
        Self::at(web3, address.0)
    }
}
