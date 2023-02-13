//! Uniswap V2 like liquidity source implementation.

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
    contracts::IUniswapLikeRouter,
    ethcontract::{H160, H256},
    hex_literal::hex,
    std::{fmt::Display, str::FromStr, sync::Arc},
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

#[derive(Debug, Clone, Copy)]
pub struct UniV2BaselineSourceParameters {
    router: H160,
    init_code_digest: H256,
    pool_reading: PoolReadingStyle,
}

#[derive(Clone, Copy, Debug, strum::EnumString, strum::Display)]
enum PoolReadingStyle {
    Default,
    Swapr,
}

pub struct UniV2BaselineSource {
    pub router: IUniswapLikeRouter,
    pub pair_provider: PairProvider,
    pub pool_fetching: Arc<dyn PoolFetching>,
}

impl UniV2BaselineSourceParameters {
    pub fn from_baseline_source(source: BaselineSource, net_version: &str) -> Option<Self> {
        use BaselineSource as BS;
        let (contract, init_code_digest, pool_reading) = match source {
            BS::None | BS::BalancerV2 | BS::ZeroEx | BS::UniswapV3 => None,
            BS::UniswapV2 => Some((
                contracts::UniswapV2Router02::raw_contract(),
                UNISWAP_INIT,
                PoolReadingStyle::Default,
            )),
            BS::Honeyswap => Some((
                contracts::HoneyswapRouter::raw_contract(),
                HONEYSWAP_INIT,
                PoolReadingStyle::Default,
            )),
            BS::SushiSwap => Some((
                contracts::SushiSwapRouter::raw_contract(),
                SUSHISWAP_INIT,
                PoolReadingStyle::Default,
            )),
            BS::Baoswap => Some((
                contracts::BaoswapRouter::raw_contract(),
                BAOSWAP_INIT,
                PoolReadingStyle::Default,
            )),
            BS::Swapr => Some((
                contracts::SwaprRouter::raw_contract(),
                SWAPR_INIT,
                PoolReadingStyle::Swapr,
            )),
        }?;
        Some(Self {
            router: contract.networks.get(net_version)?.address,
            init_code_digest: H256(init_code_digest),
            pool_reading,
        })
    }

    pub async fn into_source(&self, web3: &Web3) -> Result<UniV2BaselineSource> {
        let router = contracts::IUniswapLikeRouter::at(web3, self.router);
        let factory = router.factory().call().await.context("factory")?;
        let pair_provider = pair_provider::PairProvider {
            factory,
            init_code_digest: self.init_code_digest.0,
        };
        let pool_reader = DefaultPoolReader {
            pair_provider,
            web3: web3.clone(),
        };
        let pool_reader: Box<dyn PoolReading> = match self.pool_reading {
            PoolReadingStyle::Default => Box::new(pool_reader),
            PoolReadingStyle::Swapr => Box::new(SwaprPoolReader(pool_reader)),
        };
        let fetcher = pool_fetching::PoolFetcher {
            pool_reader,
            web3: web3.clone(),
        };
        Ok(UniV2BaselineSource {
            router,
            pair_provider,
            pool_fetching: Arc::new(fetcher),
        })
    }
}

impl Display for UniV2BaselineSourceParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}|{:?}|{}",
            self.router.0, self.init_code_digest, self.pool_reading
        )
    }
}

impl FromStr for UniV2BaselineSourceParameters {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('|');
        let router: H160 = parts
            .next()
            .context("no factory address")?
            .parse()
            .context("parse factory address")?;
        let init_code_digest: H256 = parts
            .next()
            .context("no init code digest")?
            .parse()
            .context("parse init code digest")?;
        let pool_reading = parts
            .next()
            .map(|part| part.parse().context("parse pool reading"))
            .transpose()?
            .unwrap_or(PoolReadingStyle::Default);
        Ok(Self {
            router,
            init_code_digest,
            pool_reading,
        })
    }
}

#[cfg(test)]
mod tests {
    use {super::*, model::TokenPair};

    #[test]
    fn parse_address_init() {
        let arg = "0x0000000000000000000000000000000000000001|0x0000000000000000000000000000000000000000000000000000000000000002";
        let parsed = UniV2BaselineSourceParameters::from_str(arg).unwrap();
        assert_eq!(parsed.router, H160::from_low_u64_be(1));
        assert_eq!(parsed.init_code_digest, H256::from_low_u64_be(2));
    }

    #[test]
    fn parse_pool_reading() {
        let arg = "0x0000000000000000000000000000000000000000|0x0000000000000000000000000000000000000000000000000000000000000000";
        let parsed = UniV2BaselineSourceParameters::from_str(arg).unwrap();
        assert!(matches!(parsed.pool_reading, PoolReadingStyle::Default));

        let arg = "0x0000000000000000000000000000000000000000|0x0000000000000000000000000000000000000000000000000000000000000000|Default";
        let parsed = UniV2BaselineSourceParameters::from_str(arg).unwrap();
        assert!(matches!(parsed.pool_reading, PoolReadingStyle::Default));

        let arg = "0x0000000000000000000000000000000000000000|0x0000000000000000000000000000000000000000000000000000000000000000|Swapr";
        let parsed = UniV2BaselineSourceParameters::from_str(arg).unwrap();
        assert!(matches!(parsed.pool_reading, PoolReadingStyle::Swapr));
    }

    async fn test_baseline_source(
        web3: &Web3,
        version: &str,
        source: BaselineSource,
        token0: H160,
        token1: H160,
        expected_pool_address: H160,
    ) {
        let version_ = web3.net().version().await.unwrap();
        assert_eq!(version_, version, "wrong node for test");
        let source = UniV2BaselineSourceParameters::from_baseline_source(source, version)
            .unwrap()
            .into_source(web3)
            .await
            .unwrap();
        let pair = TokenPair::new(token0, token1).unwrap();
        let pool = source.pair_provider.pair_address(&pair);
        assert_eq!(pool, expected_pool_address);
    }

    #[tokio::test]
    #[ignore]
    async fn baseline_mainnet() {
        let http = crate::ethrpc::create_env_test_transport();
        let web3 = Web3::new(http);
        let version = web3.net().version().await.unwrap();
        assert_eq!(version, "1", "test must be run with mainnet node");
        let test = |source, token0, token1, expected| {
            test_baseline_source(&web3, "1", source, token0, token1, expected)
        };

        test(
            BaselineSource::UniswapV2,
            testlib::tokens::GNO,
            testlib::tokens::WETH,
            addr!("3e8468f66d30fc99f745481d4b383f89861702c6"),
        )
        .await;
        test(
            BaselineSource::SushiSwap,
            testlib::tokens::GNO,
            testlib::tokens::WETH,
            addr!("41328fdba556c8c969418ccccb077b7b8d932aa5"),
        )
        .await;
        test(
            BaselineSource::Swapr,
            addr!("a1d65E8fB6e87b60FECCBc582F7f97804B725521"),
            testlib::tokens::WETH,
            addr!("b0Dc4B36e0B4d2e3566D2328F6806EA0B76b4F13"),
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn baseline_xdai() {
        let http = crate::ethrpc::create_env_test_transport();
        let web3 = Web3::new(http);
        let version = web3.net().version().await.unwrap();
        assert_eq!(version, "100", "test must be run with xdai node");
        let test = |source, token0, token1, expected| {
            test_baseline_source(&web3, "100", source, token0, token1, expected)
        };

        test(
            BaselineSource::Baoswap,
            addr!("7f7440c5098462f833e123b44b8a03e1d9785bab"),
            addr!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"),
            addr!("8746355882e10aae144d3709889dfaa39ff2a692"),
        )
        .await;
        test(
            BaselineSource::Honeyswap,
            addr!("71850b7e9ee3f13ab46d67167341e4bdc905eef9"),
            addr!("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
            addr!("4505b262dc053998c10685dc5f9098af8ae5c8ad"),
        )
        .await;
    }
}
