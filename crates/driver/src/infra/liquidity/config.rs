use {
    crate::{domain::eth, infra::blockchain::contracts::deployment_address},
    hex_literal::hex,
    std::collections::HashSet,
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
}

/// Uniswap V2 (and Uniswap V2 clone) liquidity fetching options.
#[derive(Clone, Copy, Debug)]
pub struct UniswapV2 {
    /// The address of the Uniswap V2 compatible router contract.
    pub router: eth::ContractAddress,
    /// The digest of the pool initialization code. This digest is used for
    /// computing the deterministic pool addresses per token pair.
    pub pool_code: eth::CodeDigest,
}

impl UniswapV2 {
    /// Returns the liquidity configuration for Uniswap V2.
    #[allow(clippy::self_named_constructors)]
    pub fn uniswap_v2(network: &eth::NetworkId) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::UniswapV2Router02::raw_contract(), network)?,
            pool_code: hex!("96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f")
                .into(),
        })
    }

    /// Returns the liquidity configuration for SushiSwap.
    pub fn sushi_swap(network: &eth::NetworkId) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::SushiSwapRouter::raw_contract(), network)?,
            pool_code: hex!("e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303")
                .into(),
        })
    }

    /// Returns the liquidity configuration for Honeyswap.
    pub fn honeyswap(network: &eth::NetworkId) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::HoneyswapRouter::raw_contract(), network)?,
            pool_code: hex!("3f88503e8580ab941773b59034fb4b2a63e86dbc031b3633a925533ad3ed2b93")
                .into(),
        })
    }

    /// Returns the liquidity configuration for Baoswap.
    pub fn baoswap(network: &eth::NetworkId) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::BaoswapRouter::raw_contract(), network)?,
            pool_code: hex!("0bae3ead48c325ce433426d2e8e6b07dac10835baec21e163760682ea3d3520d")
                .into(),
        })
    }

    /// Returns the liquidity configuration for PancakeSwap.
    pub fn pancake_swap(network: &eth::NetworkId) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::PancakeRouter::raw_contract(), network)?,
            pool_code: hex!("57224589c67f3f30a6b0d7a1b54cf3153ab84563bc609ef41dfb34f8b2974d2d")
                .into(),
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
}

impl Swapr {
    /// Returns the liquidity configuration for Swapr.
    #[allow(clippy::self_named_constructors)]
    pub fn swapr(network: &eth::NetworkId) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::SwaprRouter::raw_contract(), network)?,
            pool_code: hex!("d306a548755b9295ee49cc729e13ca4a45e00199bbd890fa146da43a50571776")
                .into(),
        })
    }
}

/// Uniswap V3 liquidity fetching options.
#[derive(Clone, Copy, Debug)]
pub struct UniswapV3 {
    /// The address of the Uniswap V3 compatible router contract.
    pub router: eth::ContractAddress,

    /// How many pools should be initialized during start up.
    pub max_pools_to_initialize: u64,
}

impl UniswapV3 {
    /// Returns the liquidity configuration for Uniswap V3.
    #[allow(clippy::self_named_constructors)]
    pub fn uniswap_v3(network: &eth::NetworkId, max_pools_to_initialize: u64) -> Option<Self> {
        Some(Self {
            router: deployment_address(contracts::UniswapV3SwapRouter::raw_contract(), network)?,
            max_pools_to_initialize,
        })
    }
}
