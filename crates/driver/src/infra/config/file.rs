use {
    crate::{
        domain::eth,
        infra::{self, config, liquidity, solver},
        util::serialize,
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
    tokio::fs,
};

/// Load the driver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path, now: infra::time::Now) -> config::Config {
    let data = fs::read(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    let config: Config = toml::de::from_slice(&data)
        .unwrap_or_else(|e| panic!("TOML syntax error while reading {path:?}: {e:?}"));

    config::Config {
        solvers: config
            .solvers
            .into_iter()
            .map(|config| solver::Config {
                endpoint: config.endpoint,
                name: config.name.into(),
                slippage: solver::Slippage {
                    relative: config.relative_slippage,
                    absolute: config.absolute_slippage.map(Into::into),
                },
                address: config.address.into(),
                now,
            })
            .collect(),
        liquidity: liquidity::Config {
            base_tokens: config
                .liquidity
                .base_tokens
                .into_iter()
                .map(eth::TokenAddress::from)
                .collect(),
            uniswap_v2: config
                .liquidity
                .uniswap_v2
                .into_iter()
                .map(|config| liquidity::config::UniswapV2 {
                    router: config.router.into(),
                    pool_code: config.pool_code.into(),
                })
                .collect(),
        },
    }
}

#[serde_as]
#[derive(Debug, Deserialize)]
struct Config {
    #[serde(rename = "solver")]
    solvers: Vec<SolverConfig>,
    #[serde(default)]
    liquidity: LiquidityConfig,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SolverConfig {
    endpoint: url::Url,
    name: String,
    #[serde_as(as = "serde_with::DisplayFromStr")]
    relative_slippage: bigdecimal::BigDecimal,
    #[serde_as(as = "Option<serialize::U256>")]
    absolute_slippage: Option<eth::U256>,
    address: eth::H160,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct LiquidityConfig {
    #[serde(default)]
    base_tokens: Vec<eth::H160>,
    #[serde(default)]
    uniswap_v2: Vec<UniswapV2Config>,
}

// TODO it would be nice to provide presets so that you can write:
// ```
// [[liquidity.uniswap-v2]]
// preset = "uniswap"
//
// [[liquidity.uniswap-v2]]
// preset = "sushiswap"
// ```
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct UniswapV2Config {
    router: eth::H160,
    pool_code: eth::H256,
}
