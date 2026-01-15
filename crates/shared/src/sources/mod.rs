//! Top-level module organizing all baseline liquidity sources.

pub mod balancer_v2;
pub mod swapr;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v3_pair_provider;

use {chain::Chain, core::panic};

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, clap::ValueEnum)]
#[clap(rename_all = "verbatim")]
pub enum BaselineSource {
    None,
    UniswapV2,
    Honeyswap,
    SushiSwap,
    BalancerV2,
    Baoswap,
    Swapr,
    ZeroEx,
    UniswapV3,
    TestnetUniswapV2,
}

pub fn defaults_for_network(chain: &Chain) -> Vec<BaselineSource> {
    match chain {
        Chain::Mainnet => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::Swapr,
            BaselineSource::BalancerV2,
            BaselineSource::ZeroEx,
            BaselineSource::UniswapV3,
        ],
        Chain::Goerli => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::BalancerV2,
        ],
        Chain::Gnosis => vec![
            BaselineSource::Honeyswap,
            BaselineSource::SushiSwap,
            BaselineSource::Baoswap,
            BaselineSource::Swapr,
        ],
        Chain::ArbitrumOne => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::Swapr,
            BaselineSource::BalancerV2,
            BaselineSource::ZeroEx,
            BaselineSource::UniswapV3,
        ],
        Chain::Base | Chain::Bnb | Chain::Avalanche | Chain::Polygon | Chain::Optimism => vec![
            BaselineSource::UniswapV2,
            BaselineSource::SushiSwap,
            BaselineSource::BalancerV2,
            BaselineSource::ZeroEx,
            BaselineSource::UniswapV3,
        ],
        Chain::Lens => vec![BaselineSource::UniswapV3],
        Chain::Linea => vec![BaselineSource::UniswapV3],
        Chain::Plasma => vec![BaselineSource::UniswapV3],
        Chain::Ink => vec![BaselineSource::UniswapV3],
        Chain::Sepolia => vec![BaselineSource::TestnetUniswapV2],
        Chain::Hardhat => panic!("unsupported baseline sources for Hardhat"),
    }
}
