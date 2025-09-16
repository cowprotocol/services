//! Euler liquidity source implementations

pub mod deposit_contract_provider;
pub mod pool_cache;
pub mod pool_fetching;

use {
    crate::{
        ethrpc::Web3,
        sources::{euler_vault::pool_fetching::{DefaultDepositContractReader, PoolFetching}, swapr::SwaprPoolReader, BaselineSource},
    }, anyhow::{Context, Result}, contracts::alloy::EulerVault, ethcontract::{H160, H256}, hex_literal::hex, std::{fmt::Display, str::FromStr, sync::Arc}
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

#[derive(Debug, Clone, Copy)]
pub struct EulerVaultBaselineSourceParameters {
}

#[derive(Clone, Copy, Debug, strum::EnumString, strum::Display)]
enum PoolReadingStyle {
    Default,
    Swapr,
}

pub struct EulerVaultBaselineSource {
    pub deposit_contract_provider: DepositContractProvider,
    pub pool_fetching: Arc<dyn PoolFetching>,
}

impl EulerVaultBaselineSourceParameters {
    pub fn from_baseline_source(source: BaselineSource, chain: &str) -> Option<Self> {
        use BaselineSource as BS;
        if source == BS::EulerVault {
            Some(Self {
                deposit_contract_provider: deposit_contract_provider::DepositContractProvider::new(),
                pool_fetching: DefaultDepositContractReader::new(),
            })
        } else {
            None
        }
    }

    pub async fn into_source(&self, web3: &Web3) -> Result<EulerVaultBaselineSource> {
        let web3 = ethrpc::instrumented::instrument_with_label(web3, "euler_vault".into());
        let router = contracts::IUniswapLikeRouter::at(&web3, self.router);
        let factory = router.factory().call().await.context("factory")?;
        let pair_provider = deposit_contract_provider::DepositContractProvider {
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
        Ok(Self {
            deposit_contract_provider: deposit_contract_provider::DepositContractProvider::new(),
            pool_fetching: fetcher, 
        })
    }
}

impl Display for EulerVaultBaselineSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}|{:?}|{}",
            self.router, self.init_code_digest, self.pool_reading
        )
    }
}

impl FromStr for EulerVaultBaselineSource {
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
