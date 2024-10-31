use {
    crate::{domain::eth, infra::blockchain::contracts::deployment_address},
    derivative::Derivative,
    hex_literal::hex,
    reqwest::Url,
    std::{collections::HashSet, time::Duration},
};

/// Configuration options for liquidity fetching.
#[derive(Clone, Debug)]
pub struct Config {
    /// Liquidity base tokens. These are additional tokens for which liquidity
    /// is always fetched, regardless of whether or not the token appears in the
    /// auction.
    pub base_tokens: HashSet<eth::TokenAddress>,

    /// The collection of Uniswap V2 compatible exchanges to fetch liquidity
    /// for.
    pub uniswap_v2: Vec<UniswapV2>,

    /// The collection of Swapr compatible exchanges to fetch liquidity for.
    pub swapr: Vec<Swapr>,

    /// The collection of Uniswap V3 compatible exchanges to fetch liquidity
    /// for.
    pub uniswap_v3: Vec<UniswapV3>,

    /// The collection of Balancer V2 compatible exchanges to fetch liquidity
    /// for.
    pub balancer_v2: Vec<BalancerV2>,

    /// 0x liquidity fetcher.
    pub zeroex: Option<ZeroEx>,
}

/// Uniswap V2 (and Uniswap V2 clone) liquidity fetching options.
#[derive(Clone, Copy, Debug)]
pub struct UniswapV2 {
    /// The address of the Uniswap V2 compatible router contract.
    pub router: eth::ContractAddress,
    /// The digest of the pool initialization code. This digest is used for
    /// computing the deterministic pool addresses per token pair.
    pub pool_code: eth::CodeDigest,
    /// How long liquidity should not be fetched for a token pair that didn't
    /// return useful liquidity before allowing to fetch it again.
    pub missing_pool_cache_time: Duration,
}

impl UniswapV2 {
    /// Returns the liquidity configuration for Uniswap V2.
    #[allow(clippy::self_named_constructors)]
    pub fn uniswap_v2(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::UniswapV2Router02::raw_contract(), chain)?,
            pool_code: hex!("96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for SushiSwap.
    pub fn sushi_swap(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::SushiSwapRouter::raw_contract(), chain)?,
            pool_code: hex!("e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for Honeyswap.
    pub fn honeyswap(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::HoneyswapRouter::raw_contract(), chain)?,
            pool_code: hex!("3f88503e8580ab941773b59034fb4b2a63e86dbc031b3633a925533ad3ed2b93")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for Baoswap.
    pub fn baoswap(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::BaoswapRouter::raw_contract(), chain)?,
            pool_code: hex!("0bae3ead48c325ce433426d2e8e6b07dac10835baec21e163760682ea3d3520d")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for PancakeSwap.
    pub fn pancake_swap(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::PancakeRouter::raw_contract(), chain)?,
            pool_code: hex!("57224589c67f3f30a6b0d7a1b54cf3153ab84563bc609ef41dfb34f8b2974d2d")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for liquidity sources only used on
    /// test networks.
    pub fn testnet_uniswapv2(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::TestnetUniswapV2Router02::raw_contract(), chain)?,
            pool_code: hex!("0efd7612822d579e24a8851501d8c2ad854264a1050e3dfcee8afcca08f80a86")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }
}

/// Swapr (Uniswap V2 clone with a twist) liquidity fetching options.
#[derive(Clone, Copy, Debug)]
pub struct Swapr {
    /// The address of the Swapr compatible router contract.
    pub router: eth::ContractAddress,
    /// The digest of the pool initialization code. This digest is used for
    /// computing the deterministic pool addresses per token pair.
    pub pool_code: eth::CodeDigest,
    /// How long liquidity should not be fetched for a token pair that didn't
    /// return useful liquidity before allowing to fetch it again.
    pub missing_pool_cache_time: Duration,
}

impl Swapr {
    /// Returns the liquidity configuration for Swapr.
    #[allow(clippy::self_named_constructors)]
    pub fn swapr(chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::SwaprRouter::raw_contract(), chain)?,
            pool_code: hex!("d306a548755b9295ee49cc729e13ca4a45e00199bbd890fa146da43a50571776")
                .into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }
}

/// Uniswap V3 liquidity fetching options.
#[derive(Clone, Debug)]
pub struct UniswapV3 {
    /// The address of the Uniswap V3 compatible router contract.
    pub router: eth::ContractAddress,

    /// How many pools should be initialized during start up.
    pub max_pools_to_initialize: usize,

    /// The URL used to connect to uniswap v3 subgraph client.
    pub graph_url: Url,
}

impl UniswapV3 {
    /// Returns the liquidity configuration for Uniswap V3.
    #[allow(clippy::self_named_constructors)]
    pub fn uniswap_v3(graph_url: &Url, chain: chain::Id) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::UniswapV3SwapRouter::raw_contract(), chain)?,
            max_pools_to_initialize: 100,
            graph_url: graph_url.clone(),
        })
    }
}

/// Balancer V2 liquidity fetching options.
#[derive(Clone, Debug)]
pub struct BalancerV2 {
    /// The address of the Uniswap V3 compatible router contract.
    pub vault: eth::ContractAddress,

    /// Weighted pool factory addresses.
    pub weighted: Vec<eth::ContractAddress>,

    /// Weighted pool factory v3+ addresses.
    pub weighted_v3plus: Vec<eth::ContractAddress>,

    /// Stable pool factory addresses.
    pub stable: Vec<eth::ContractAddress>,

    /// Liquidity bootstrapping pool factory addresses.
    pub liquidity_bootstrapping: Vec<eth::ContractAddress>,

    /// Composable stable pool factory addresses.
    pub composable_stable: Vec<eth::ContractAddress>,

    /// Deny listed Balancer V2 pools.
    ///
    /// Since pools allow for custom controllers and logic, it is possible for
    /// pools to get "bricked". This configuration allows those pools to be
    /// ignored.
    pub pool_deny_list: Vec<eth::H256>,

    /// The base URL used to connect to balancer v2 subgraph client.
    pub graph_url: Url,
}

impl BalancerV2 {
    /// Returns the liquidity configuration for Balancer V2.
    #[allow(clippy::self_named_constructors)]
    pub fn balancer_v2(graph_url: &Url, chain: chain::Id) -> Option<Self> {
        let factory_addresses =
            |contracts: &[&ethcontract::Contract]| -> Vec<eth::ContractAddress> {
                contracts
                    .iter()
                    .copied()
                    .filter_map(|c| deployment_address(c, chain))
                    .collect()
            };

        Some(Self {
            vault: deployment_address(contracts::BalancerV2Vault::raw_contract(), chain)?,
            weighted: factory_addresses(&[
                contracts::BalancerV2WeightedPoolFactory::raw_contract(),
                contracts::BalancerV2WeightedPool2TokensFactory::raw_contract(),
            ]),
            weighted_v3plus: factory_addresses(&[
                contracts::BalancerV2WeightedPoolFactoryV3::raw_contract(),
                contracts::BalancerV2WeightedPoolFactoryV4::raw_contract(),
            ]),
            stable: factory_addresses(&[contracts::BalancerV2StablePoolFactoryV2::raw_contract()]),
            liquidity_bootstrapping: factory_addresses(&[
                contracts::BalancerV2LiquidityBootstrappingPoolFactory::raw_contract(),
                contracts::BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory::raw_contract(),
            ]),
            composable_stable: factory_addresses(&[
                contracts::BalancerV2ComposableStablePoolFactory::raw_contract(),
                contracts::BalancerV2ComposableStablePoolFactoryV3::raw_contract(),
                contracts::BalancerV2ComposableStablePoolFactoryV4::raw_contract(),
                contracts::BalancerV2ComposableStablePoolFactoryV5::raw_contract(),
                contracts::BalancerV2ComposableStablePoolFactoryV6::raw_contract(),
            ]),
            pool_deny_list: Vec::new(),
            graph_url: graph_url.clone(),
        })
    }
}

/// ZeroEx liquidity fetching options.
#[derive(Clone, Derivative)]
#[derivative(Debug)]
pub struct ZeroEx {
    pub base_url: String,
    #[derivative(Debug = "ignore")]
    pub api_key: Option<String>,
    pub http_timeout: Duration,
}
