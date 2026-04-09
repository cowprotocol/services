use {
    crate::{
        domain::eth,
        infra::{config::dex::file, dex::bitget},
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// The base URL for the Bitget swap API.
    #[serde(default = "default_endpoint")]
    #[serde_as(as = "serde_with::DisplayFromStr")]
    endpoint: reqwest::Url,

    /// Chain ID used to automatically determine contract addresses.
    chain_id: eth::ChainId,

    /// Bitget API credentials.
    credentials: BitgetCredentialsConfig,

    /// Partner code sent in the `Partner-Code` header.
    #[serde(default = "default_partner_code")]
    partner_code: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct BitgetCredentialsConfig {
    /// Bitget API key.
    api_key: String,

    /// Bitget API secret for signing requests.
    api_secret: String,
}

#[allow(clippy::from_over_into)]
impl Into<bitget::BitgetCredentialsConfig> for BitgetCredentialsConfig {
    fn into(self) -> bitget::BitgetCredentialsConfig {
        bitget::BitgetCredentialsConfig {
            api_key: self.api_key,
            api_secret: self.api_secret,
        }
    }
}

fn default_partner_code() -> String {
    "cowswap".to_string()
}

fn default_endpoint() -> reqwest::Url {
    bitget::DEFAULT_ENDPOINT.parse().unwrap()
}

/// Load the Bitget solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    super::Config {
        bitget: bitget::Config {
            endpoint: config.endpoint,
            chain_id: config.chain_id,
            credentials: config.credentials.into(),
            partner_code: config.partner_code,
            block_stream: base.block_stream.clone(),
            settlement_contract: base.contracts.settlement,
        },
        base,
    }
}
