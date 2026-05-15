use {serde::Deserialize, serde_with::serde_as, url::Url};

/// Default number of winning solutions selected by the local arbitrator.
///
/// Once pod takes real auction traffic this acts as a protocol parameter:
/// changing it has the same blast radius as a parameter hardfork. Exposed
/// in config from the first version so a future change is configurable
/// without a release.
const DEFAULT_MAX_WINNERS: usize = 10;

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub endpoint: Url,
    pub auction_contract_address: pod_sdk::alloy_primitives::Address,
    /// Maximum number of winning solutions selected by the local arbitrator
    /// when running shadow-mode arbitration over fetched pod bids.
    #[serde(default = "default_max_winners")]
    pub max_winners: usize,
}

fn default_max_winners() -> usize {
    DEFAULT_MAX_WINNERS
}
