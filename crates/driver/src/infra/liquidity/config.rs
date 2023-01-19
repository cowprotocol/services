use {crate::domain::eth, std::collections::HashSet};

/// Configuration options for liquidity fetching.
#[derive(Debug)]
pub struct Config {
    /// Liquidity base tokens. These are additional tokens for which liquidity
    /// is always fetched, regardless of whether or not the token appears in the
    /// auction.
    pub base_tokens: HashSet<eth::TokenAddress>,

    /// The collection of Uniswap V2 compatible exchanges to fetch liquidity
    /// for.
    pub uniswap_v2: Vec<UniswapV2>,
}

/// Uniswap V2 (and Uniswap V2 clone) liquidity fetching options.
#[derive(Debug)]
pub struct UniswapV2 {
    /// The address of the Uniswap V2 compatible router contract.
    pub router: eth::ContractAddress,
    /// The digest of the pool initialization code. This digest is used for
    /// computing the deterministic pool addresses per token pair.
    pub pool_code: eth::CodeDigest,
}
