use {alloy_primitives::Address, serde::Deserialize, std::path::Path, tokio::fs, url::Url};

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub chain_id: u64,
    pub orderbook_url: Url,
    pub byos_url: Url,
    pub node_url: Url,
    #[serde(with = "humantime")]
    pub poll_interval: std::time::Duration,
    pub private_key: String,
    pub solver: SolverConfig,
    pub uniswap_v2: UniswapV2Config,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct SolverConfig {
    pub base_tokens: Vec<Address>,
    pub max_hops: usize,
    #[serde(default = "default_max_partial_attempts")]
    pub max_partial_attempts: usize,
    pub native_token_price_estimation_amount: alloy_primitives::U256,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct UniswapV2Config {
    pub router: Address,
}

pub async fn load(path: &Path) -> Config {
    let data = fs::read_to_string(path)
        .await
        .unwrap_or_else(|e| panic!("I/O error while reading {path:?}: {e:?}"));
    toml::de::from_str::<Config>(&data)
        .unwrap_or_else(|e| panic!("invalid subsolver config {path:?}: {e:?}"))
}

fn default_max_partial_attempts() -> usize {
    1
}

mod humantime {
    use {
        serde::{Deserialize, Deserializer},
        std::time::Duration,
    };

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Duration, D::Error> {
        let s = String::deserialize(d)?;
        humantime::parse_duration(&s).map_err(serde::de::Error::custom)
    }
}
