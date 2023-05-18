use {
    crate::{
        domain::eth,
        infra::{config::dex::file, contracts, dex::oneinch},
        util::serialize,
    },
    serde::Deserialize,
    serde_with::serde_as,
    std::path::Path,
};

#[serde_as]
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Config {
    /// Chain ID used to automatically determine the address of the settlement
    /// contract and for metrics.
    #[serde_as(as = "serialize::ChainId")]
    chain_id: eth::ChainId,

    /// The URL endpoint for the 1inch API.
    #[serde_as(as = "Option<serde_with::DisplayFromStr>")]
    endpoint: Option<reqwest::Url>,

    /// The 1Inch liquidity sources to consider when swapping.
    #[serde(flatten)]
    liquidity: Liquidity,

    /// The referrer address to use. Referrers are entitled to a portion of
    /// the positive slippage that 1Inch collects.
    referrer: Option<eth::H160>,

    // The following configuration options tweak the complexity of the 1Inch
    // route that the API returns. Unfortunately, the exact definition (and
    // what each field actually controls) is fairly opaque and not well
    // documented.
    main_route_parts: Option<u32>,
    connector_tokens: Option<u32>,
    complexity_level: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields, untagged)]
enum Liquidity {
    Any,
    Only { include_liquidity: Vec<String> },
    Exclude { exclude_liquidity: Vec<String> },
}

/// Load the 1inch solver configuration from a TOML file.
///
/// # Panics
///
/// This method panics if the config is invalid or on I/O errors.
pub async fn load(path: &Path) -> super::Config {
    let (base, config) = file::load::<Config>(path).await;

    let settlement = contracts::Contracts::for_chain(config.chain_id).settlement;

    super::Config {
        oneinch: oneinch::Config {
            settlement,
            endpoint: config.endpoint,
            liquidity: match config.liquidity {
                Liquidity::Any => oneinch::Liquidity::Any,
                Liquidity::Only { include_liquidity } => {
                    oneinch::Liquidity::Only(include_liquidity)
                }
                Liquidity::Exclude { exclude_liquidity } => {
                    oneinch::Liquidity::Exclude(exclude_liquidity)
                }
            },
            referrer: config.referrer,
            main_route_parts: config.main_route_parts,
            connector_tokens: config.connector_tokens,
            complexity_level: config.complexity_level,
        },
        base,
    }
}
