pub mod bitget;
pub mod file;
pub mod okx;

use {
    crate::domain::{
        dex::{minimum_surplus::MinimumSurplusLimits, slippage::SlippageLimits},
        eth,
    },
    alloy::primitives::Address,
    ethrpc::block_stream::CurrentBlockWatcher,
    std::num::NonZeroUsize,
};

#[derive(Clone)]
pub struct Contracts {
    pub settlement: Address,
    pub authenticator: Address,
}

#[derive(Clone)]
pub struct Config {
    pub node_url: reqwest::Url,
    pub contracts: Contracts,
    pub slippage: SlippageLimits,
    pub minimum_surplus: MinimumSurplusLimits,
    pub concurrent_requests: NonZeroUsize,
    pub smallest_partial_fill: eth::Ether,
    pub rate_limiting_strategy: configs::rate_limit::Strategy,
    pub gas_offset: eth::Gas,
    pub block_stream: Option<CurrentBlockWatcher>,
    pub internalize_interactions: bool,
}
