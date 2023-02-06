//! Uniswap V2 baseline liquidity source implementation.
pub mod pair_provider;
pub mod pool_cache;
pub mod pool_fetching;

use {
    self::{
        pair_provider::PairProvider,
        pool_fetching::{DefaultPoolReader, PoolFetching, PoolReading},
    },
    crate::{
        ethrpc::Web3,
        sources::{swapr::reader::SwaprPoolReader, BaselineSource},
    },
    anyhow::{Context, Result},
    ethcontract::{H160, H256},
    hex_literal::hex,
    std::sync::Arc,
};

pub const UNISWAP_INIT: [u8; 32] =
    hex!("96e8ac4277198ff8b6f785478aa9a39f403cb768dd02cbee326c3e7da348845f");
pub const HONEYSWAP_INIT: [u8; 32] =
    hex!("3f88503e8580ab941773b59034fb4b2a63e86dbc031b3633a925533ad3ed2b93");
pub const SUSHISWAP_INIT: [u8; 32] =
    hex!("e18a34eb0e04b04f7a0ac29a6e80748dca96319b42c54d679cb821dca90c6303");
pub const BAOSWAP_INIT: [u8; 32] =
    hex!("0bae3ead48c325ce433426d2e8e6b07dac10835baec21e163760682ea3d3520d");
pub const SWAPR_INIT: [u8; 32] =
    hex!("d306a548755b9295ee49cc729e13ca4a45e00199bbd890fa146da43a50571776");

/// If the baseline source is uniswapv2-like, returns the address of the router
/// contract and the init code digest needed for calculating pair addresses.
pub fn from_baseline_source(source: BaselineSource, net_version: &str) -> Option<(H160, H256)> {
    let (contract, init_code_digest) = match source {
        BaselineSource::BalancerV2 => None,
        BaselineSource::ZeroEx => None,
        BaselineSource::UniswapV3 => None,
        BaselineSource::UniswapV2 => {
            Some((contracts::UniswapV2Router02::raw_contract(), UNISWAP_INIT))
        }
        BaselineSource::Honeyswap => {
            Some((contracts::HoneyswapRouter::raw_contract(), HONEYSWAP_INIT))
        }
        BaselineSource::SushiSwap => {
            Some((contracts::SushiSwapRouter::raw_contract(), SUSHISWAP_INIT))
        }
        BaselineSource::Baoswap => Some((contracts::BaoswapRouter::raw_contract(), BAOSWAP_INIT)),
        BaselineSource::Swapr => Some((contracts::SwaprRouter::raw_contract(), SWAPR_INIT)),
    }?;
    let address = contract.networks.get(net_version)?.address;
    Some((address, H256(init_code_digest)))
}

/// If the baseline source is uniswapv2-like, returns the corresponding
/// PoolReading implementation.
pub fn pool_reader(
    source: BaselineSource,
    pair_provider: PairProvider,
    web3: &Web3,
) -> Option<Box<dyn PoolReading>> {
    let default_reader = DefaultPoolReader {
        pair_provider,
        web3: web3.clone(),
    };
    match source {
        BaselineSource::BalancerV2 => None,
        BaselineSource::ZeroEx => None,
        BaselineSource::UniswapV3 => None,
        BaselineSource::UniswapV2 => Some(Box::new(default_reader)),
        BaselineSource::Honeyswap => Some(Box::new(default_reader)),
        BaselineSource::SushiSwap => Some(Box::new(default_reader)),
        BaselineSource::Baoswap => Some(Box::new(default_reader)),
        BaselineSource::Swapr => Some(Box::new(SwaprPoolReader(default_reader))),
    }
}

pub async fn uniswap_like_liquidity_source(
    source: BaselineSource,
    web3: &Web3,
) -> Result<Option<(PairProvider, Arc<dyn PoolFetching>)>> {
    let net_version = web3.net().version().await.context("net_version")?;
    let (router, init_code_digest) = match from_baseline_source(source, &net_version) {
        Some(inner) => inner,
        None => return Ok(None),
    };
    let router = contracts::IUniswapLikeRouter::at(web3, router);
    let factory = router.factory().call().await.context("factory")?;
    let provider = pair_provider::PairProvider {
        factory,
        init_code_digest: init_code_digest.0,
    };
    let pool_reader = match pool_reader(source, provider, web3) {
        Some(inner) => inner,
        None => return Ok(None),
    };
    let fetcher = pool_fetching::PoolFetcher {
        pool_reader,
        web3: web3.clone(),
    };
    Ok(Some((provider, Arc::new(fetcher))))
}
