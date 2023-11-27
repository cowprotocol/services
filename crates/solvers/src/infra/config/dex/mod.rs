pub mod balancer;
mod file;
pub mod oneinch;
pub mod paraswap;
pub mod zeroex;

use {
    crate::{
        boundary::rate_limiter::RateLimitingStrategy,
        domain::{dex::slippage, eth, Risk},
    },
    std::num::NonZeroUsize,
};

#[derive(Clone)]
pub struct Contracts {
    pub settlement: eth::ContractAddress,
    pub authenticator: eth::ContractAddress,
}

#[derive(Clone)]
pub struct Config {
    pub node_url: reqwest::Url,
    pub contracts: Contracts,
    pub slippage: slippage::Limits,
    pub concurrent_requests: NonZeroUsize,
    pub smallest_partial_fill: eth::Ether,
    pub risk: Risk,
    pub rate_limiting_strategy: RateLimitingStrategy,
}
