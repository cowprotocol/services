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
        sources::{BaselineSource, swapr::SwaprPoolReader},
    },
    alloy::primitives::{Address, B256},
    anyhow::{Context, Result},
    contracts::alloy::IUniswapLikeRouter,
    ethrpc::alloy::ProviderLabelingExt,
    hex_literal::hex,
    std::{fmt::Display, str::FromStr, sync::Arc},
};

// How to compute for unknown contracts
// Find a pair creation transaction and open it with Tenderly on the debugger
// page. Example:
// https://dashboard.tenderly.co/tx/sepolia/0x4d31daa9e74b96a5c9a780cf8839b115ac25127b17226ecb1ad6e7f244fd1c8f/debugger?trace=0.1
// Find the CREATE2 step and take the  "input" value in the debugger box; this
// is the init code. Trim 0x and hash the resulting hex-encoded bytestring, for
// example with `xxd -ps -r < ./initcode.txt | openssl dgst -keccak-256` (with
// Openssl version ≥3.2) or https://emn178.github.io/online-tools/keccak_256.html
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
pub const TESTNET_UNISWAP_INIT: [u8; 32] =
    hex!("0efd7612822d579e24a8851501d8c2ad854264a1050e3dfcee8afcca08f80a86");

#[derive(Clone, Copy, Debug, strum::EnumString, strum::Display)]
enum PoolReadingStyle {
    Default,
    Swapr,
}

pub struct UniV2BaselineSource {
    pub router: IUniswapLikeRouter::Instance,
    pub pair_provider: PairProvider,
    pub pool_fetching: Arc<dyn PoolFetching>,
}

#[derive(Debug, Clone, Copy)]
pub struct UniV2BaselineSourceParameters {
    router: Address,
    init_code_digest: B256,
    pool_reading: PoolReadingStyle,
}

impl UniV2BaselineSourceParameters {
    pub fn from_baseline_source(source: BaselineSource, chain: &str) -> Option<Self> {
        use BaselineSource as BS;

        let chain_id = chain.parse::<u64>().expect("chain id should be an integer");

        match source {
            BS::None | BS::BalancerV2 | BS::ZeroEx | BS::UniswapV3 => None,
            BS::UniswapV2 => Some(Self {
                router: contracts::alloy::UniswapV2Router02::deployment_address(&chain_id)?,
                init_code_digest: UNISWAP_INIT.into(),
                pool_reading: PoolReadingStyle::Default,
            }),
            BS::Honeyswap => Some(Self {
                router: contracts::alloy::HoneyswapRouter::deployment_address(&chain_id)?,
                init_code_digest: HONEYSWAP_INIT.into(),
                pool_reading: PoolReadingStyle::Default,
            }),
            BS::SushiSwap => Some(Self {
                router: contracts::alloy::SushiSwapRouter::deployment_address(&chain_id)?,
                init_code_digest: SUSHISWAP_INIT.into(),
                pool_reading: PoolReadingStyle::Default,
            }),
            BS::Swapr => Some(Self {
                router: contracts::alloy::SwaprRouter::deployment_address(&chain_id)?,
                init_code_digest: SWAPR_INIT.into(),
                pool_reading: PoolReadingStyle::Swapr,
            }),
            BS::TestnetUniswapV2 => Some(Self {
                router: contracts::alloy::TestnetUniswapV2Router02::deployment_address(&chain_id)?,
                init_code_digest: TESTNET_UNISWAP_INIT.into(),
                pool_reading: PoolReadingStyle::Default,
            }),
            BS::Baoswap => Some(Self {
                router: contracts::alloy::BaoswapRouter::deployment_address(&chain_id)?,
                init_code_digest: BAOSWAP_INIT.into(),
                pool_reading: PoolReadingStyle::Default,
            }),
        }
    }

    pub async fn into_source(&self, web3: &Web3) -> Result<UniV2BaselineSource> {
        let web3 = web3.labeled("uniswapV2");
        let router =
            contracts::alloy::IUniswapLikeRouter::Instance::new(self.router, web3.alloy.clone());
        let factory = router.factory().call().await.context("factory")?;
        let pair_provider = pair_provider::PairProvider {
            factory,
            init_code_digest: self.init_code_digest.0,
        };
        let pool_reader = DefaultPoolReader::new(web3.clone(), pair_provider);
        let pool_reader: Box<dyn PoolReading> = match self.pool_reading {
            PoolReadingStyle::Default => Box::new(pool_reader),
            PoolReadingStyle::Swapr => Box::new(SwaprPoolReader(pool_reader)),
        };
        let fetcher =
            pool_fetching::PoolFetcher::new(pool_reader, web3.clone(), Default::default());
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
            self.router, self.init_code_digest, self.pool_reading
        )
    }
}

impl FromStr for UniV2BaselineSourceParameters {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('|');
        let router: Address = parts
            .next()
            .context("no factory address")?
            .parse()
            .context("parse factory address")?;
        let init_code_digest: B256 = parts
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
    use {
        super::*,
        crate::recent_block_cache::Block,
        alloy::{
            primitives::{Address, B256, address},
            providers::Provider,
        },
        maplit::hashset,
        model::TokenPair,
    };

    #[test]
    fn parse_address_init() {
        let arg = "0x0000000000000000000000000000000000000001|0x0000000000000000000000000000000000000000000000000000000000000002";
        let parsed = UniV2BaselineSourceParameters::from_str(arg).unwrap();
        assert_eq!(parsed.init_code_digest, B256::with_last_byte(2));
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
        token0: Address,
        token1: Address,
        expected_pool_address: Address,
    ) {
        let version_ = web3.alloy.get_chain_id().await.unwrap().to_string();
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
        let web3 = ethrpc::Web3::new_from_env();
        let version = web3.alloy.get_chain_id().await.unwrap().to_string();
        assert_eq!(version, "1", "test must be run with mainnet node");
        let test = |source, token0, token1, expected| {
            test_baseline_source(&web3, "1", source, token0, token1, expected)
        };

        test(
            BaselineSource::UniswapV2,
            testlib::tokens::GNO,
            testlib::tokens::WETH,
            address!("3e8468f66d30fc99f745481d4b383f89861702c6"),
        )
        .await;
        test(
            BaselineSource::SushiSwap,
            testlib::tokens::GNO,
            testlib::tokens::WETH,
            address!("41328fdba556c8c969418ccccb077b7b8d932aa5"),
        )
        .await;
        test(
            BaselineSource::Swapr,
            address!("a1d65E8fB6e87b60FECCBc582F7f97804B725521"),
            testlib::tokens::WETH,
            address!("b0Dc4B36e0B4d2e3566D2328F6806EA0B76b4F13"),
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn baseline_sepolia() {
        let web3 = ethrpc::Web3::new_from_env();
        let version = web3.alloy.get_chain_id().await.unwrap().to_string();
        assert_eq!(version, "11155111", "test must be run with mainnet node");
        let test = |source, token0, token1, expected| {
            test_baseline_source(&web3, "11155111", source, token0, token1, expected)
        };

        // https://sepolia.etherscan.io/tx/0x4d31daa9e74b96a5c9a780cf8839b115ac25127b17226ecb1ad6e7f244fd1c8f
        test(
            BaselineSource::TestnetUniswapV2,
            address!("fff9976782d46cc05630d1f6ebab18b2324d6b14"),
            address!("7c43482436624585c27cc9f804e53463d5a37aba"),
            address!("84A1CE0e56500D51a6a6e2559567007E26dc8a7C"),
        )
        .await;
    }

    #[tokio::test]
    #[ignore]
    async fn baseline_xdai() {
        let web3 = ethrpc::Web3::new_from_env();
        let version = web3.alloy.get_chain_id().await.unwrap().to_string();
        assert_eq!(version, "100", "test must be run with xdai node");
        let test = |source, token0, token1, expected| {
            test_baseline_source(&web3, "100", source, token0, token1, expected)
        };

        test(
            BaselineSource::Baoswap,
            address!("7f7440c5098462f833e123b44b8a03e1d9785bab"),
            address!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d"),
            address!("8746355882e10aae144d3709889dfaa39ff2a692"),
        )
        .await;
        test(
            BaselineSource::Honeyswap,
            address!("71850b7e9ee3f13ab46d67167341e4bdc905eef9"),
            address!("e91d153e0b41518a2ce8dd3d7944fa863463a97d"),
            address!("4505b262dc053998c10685dc5f9098af8ae5c8ad"),
        )
        .await;
    }

    const GNOSIS_CHAIN_WETH: Address = address!("6A023CCd1ff6F2045C3309768eAd9E68F978f6e1");
    const GNOSIS_CHAIN_WXDAI: Address = address!("e91D153E0b41518A2Ce8Dd3D7944Fa863463a97d");

    #[tokio::test]
    #[ignore]
    async fn fetch_baoswap_pool() {
        let web3 = Web3::new_from_env();
        let version = web3.alloy.get_chain_id().await.unwrap().to_string();
        let pool_fetcher =
            UniV2BaselineSourceParameters::from_baseline_source(BaselineSource::Baoswap, &version)
                .unwrap()
                .into_source(&web3)
                .await
                .unwrap()
                .pool_fetching;
        let pool = pool_fetcher
            .fetch(
                hashset! {
                    TokenPair::new(
                        GNOSIS_CHAIN_WETH,
                        GNOSIS_CHAIN_WXDAI,
                    )
                    .unwrap(),
                },
                Block::Recent,
            )
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        println!("WETH <> wxDAI pool: {pool:#?}");
        assert_eq!(
            pool.address,
            address!("8c36f7ca02d50bf8e705f582328b873acbe9438d")
        );
    }

    #[tokio::test]
    #[ignore]
    async fn fetch_honeyswap_pool() {
        let web3 = Web3::new_from_env();
        let version = web3.alloy.get_chain_id().await.unwrap().to_string();
        let pool_fetcher = UniV2BaselineSourceParameters::from_baseline_source(
            BaselineSource::Honeyswap,
            &version,
        )
        .unwrap()
        .into_source(&web3)
        .await
        .unwrap()
        .pool_fetching;
        let pool = pool_fetcher
            .fetch(
                hashset! {
                    TokenPair::new(
                        GNOSIS_CHAIN_WETH,
                        GNOSIS_CHAIN_WXDAI,
                    )
                    .unwrap(),
                },
                Block::Recent,
            )
            .await
            .unwrap()
            .into_iter()
            .next()
            .unwrap();

        println!("WETH <> wxDAI pool: {pool:#?}");
        assert_eq!(
            pool.address,
            address!("7bea4af5d425f2d4485bdad1859c88617df31a67")
        );
    }
}
