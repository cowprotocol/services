use {
    alloy::primitives::Address,
    chain::Chain,
    contracts::alloy::BalancerV2Vault,
    derive_more::Debug,
    hex_literal::hex,
    reqwest::Url,
    shared::{
        domain::eth::{self, ContractAddress},
        sources::uniswap_v2::{
            BAOSWAP_INIT,
            HONEYSWAP_INIT,
            SUSHISWAP_INIT,
            SWAPR_INIT,
            TESTNET_UNISWAP_INIT,
            UNISWAP_INIT,
        },
    },
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
    #[expect(clippy::self_named_constructors)]
    pub fn uniswap_v2(chain: Chain) -> Option<Self> {
        Some(Self {
            router: ContractAddress::from(contracts::alloy::UniswapV2Router02::deployment_address(
                &chain.id(),
            )?),
            pool_code: UNISWAP_INIT.into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for SushiSwap.
    pub fn sushi_swap(chain: Chain) -> Option<Self> {
        Some(Self {
            router: ContractAddress::from(contracts::alloy::SushiSwapRouter::deployment_address(
                &chain.id(),
            )?),
            pool_code: SUSHISWAP_INIT.into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for Honeyswap.
    pub fn honeyswap(chain: Chain) -> Option<Self> {
        Some(Self {
            router: ContractAddress::from(contracts::alloy::BaoswapRouter::deployment_address(
                &chain.id(),
            )?),
            pool_code: HONEYSWAP_INIT.into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for Baoswap.
    pub fn baoswap(chain: Chain) -> Option<Self> {
        Some(Self {
            router: ContractAddress::from(contracts::alloy::BaoswapRouter::deployment_address(
                &chain.id(),
            )?),
            pool_code: BAOSWAP_INIT.into(),
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for PancakeSwap.
    pub fn pancake_swap(chain: Chain) -> Option<Self> {
        let pool_code = match chain {
            Chain::Bnb => hex!("00fb7f630766e6a796048ea87d01acd3068e8ff67d078148a3fa3f4a84f69bd5"),
            _ => hex!("57224589c67f3f30a6b0d7a1b54cf3153ab84563bc609ef41dfb34f8b2974d2d"),
        }
        .into();
        Some(Self {
            router: ContractAddress::from(contracts::alloy::PancakeRouter::deployment_address(
                &chain.id(),
            )?),
            pool_code,
            missing_pool_cache_time: Duration::from_secs(60 * 60),
        })
    }

    /// Returns the liquidity configuration for liquidity sources only used on
    /// test networks.
    pub fn testnet_uniswapv2(chain: Chain) -> Option<Self> {
        Some(Self {
            router: ContractAddress::from(
                contracts::alloy::TestnetUniswapV2Router02::deployment_address(&chain.id())?,
            ),
            pool_code: TESTNET_UNISWAP_INIT.into(),
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
    #[expect(clippy::self_named_constructors)]
    pub fn swapr(chain: Chain) -> Option<Self> {
        Some(Self {
            router: ContractAddress::from(contracts::alloy::SwaprRouter::deployment_address(
                &chain.id(),
            )?),
            pool_code: SWAPR_INIT.into(),
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

    /// How often the liquidity source should be reinitialized to
    /// become aware of new pools.
    pub reinit_interval: Option<Duration>,

    /// How many pool IDs can be present in a where clause of a Tick query at
    /// once. Some subgraphs are overloaded and throw errors when there are
    /// too many.
    pub max_pools_per_tick_query: usize,
}

impl UniswapV3 {
    /// Returns the liquidity configuration for Uniswap V3.
    #[expect(clippy::self_named_constructors)]
    pub fn uniswap_v3(
        graph_url: &Url,
        chain: Chain,
        max_pools_per_tick_query: usize,
    ) -> Option<Self> {
        Some(Self {
            router: contracts::alloy::UniswapV3SwapRouterV2::deployment_address(&chain.id())?
                .into(),
            max_pools_to_initialize: 100,
            graph_url: graph_url.clone(),
            reinit_interval: None,
            max_pools_per_tick_query,
        })
    }
}

/// Balancer V2 liquidity fetching options.
#[derive(Clone, Debug)]
pub struct BalancerV2 {
    /// The address of the Uniswap V3 compatible router contract.
    pub vault: eth::ContractAddress,

    /// Weighted pool factory addresses.
    pub weighted: Vec<Address>,

    /// Weighted pool factory v3+ addresses.
    pub weighted_v3plus: Vec<Address>,

    /// Stable pool factory addresses.
    pub stable: Vec<Address>,

    /// Liquidity bootstrapping pool factory addresses.
    pub liquidity_bootstrapping: Vec<Address>,

    /// Composable stable pool factory addresses.
    pub composable_stable: Vec<Address>,

    /// Deny listed Balancer V2 pools.
    ///
    /// Since pools allow for custom controllers and logic, it is possible for
    /// pools to get "bricked". This configuration allows those pools to be
    /// ignored.
    pub pool_deny_list: Vec<eth::B256>,

    /// The base URL used to connect to balancer v2 subgraph client.
    pub graph_url: Url,

    /// How often the liquidty source should be re-initialized to become
    /// aware of new pools.
    pub reinit_interval: Option<Duration>,
}

impl BalancerV2 {
    /// Returns the liquidity configuration for Balancer V2.
    #[expect(clippy::self_named_constructors)]
    pub fn balancer_v2(graph_url: &Url, chain: Chain) -> Option<Self> {
        macro_rules! address_for {
            ( $chain:expr, [ $( $($p:ident)::+ ),* $(,)? ] ) => {{
                let arr = [ $({
                    $($p)::+::deployment_address(&$chain.id())
                }),* ];
                arr.into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            }};
        }

        Some(Self {
            vault: ContractAddress(BalancerV2Vault::deployment_address(&chain.id())?),
            weighted: address_for!(
                chain,
                [
                    contracts::alloy::BalancerV2WeightedPoolFactory,
                    contracts::alloy::BalancerV2WeightedPool2TokensFactory,
                ]
            ),
            weighted_v3plus: address_for!(
                chain,
                [
                    contracts::alloy::BalancerV2WeightedPoolFactoryV3,
                    contracts::alloy::BalancerV2WeightedPoolFactoryV4,
                ]
            ),
            stable: address_for!(chain, [contracts::alloy::BalancerV2StablePoolFactoryV2,]),
            liquidity_bootstrapping: address_for!(
                chain,
                [
                    contracts::alloy::BalancerV2LiquidityBootstrappingPoolFactory,
                    contracts::alloy::BalancerV2NoProtocolFeeLiquidityBootstrappingPoolFactory,
                ]
            ),
            composable_stable: address_for!(
                chain,
                [
                    contracts::alloy::BalancerV2ComposableStablePoolFactory,
                    contracts::alloy::BalancerV2ComposableStablePoolFactoryV3,
                    contracts::alloy::BalancerV2ComposableStablePoolFactoryV4,
                    contracts::alloy::BalancerV2ComposableStablePoolFactoryV5,
                    contracts::alloy::BalancerV2ComposableStablePoolFactoryV6,
                ]
            ),
            pool_deny_list: Vec::new(),
            graph_url: graph_url.clone(),
            reinit_interval: None,
        })
    }
}

/// ZeroEx liquidity fetching options.
#[derive(Clone, Debug)]
pub struct ZeroEx {
    pub base_url: String,
    #[debug(ignore)]
    pub api_key: Option<String>,
    pub http_timeout: Duration,
}
