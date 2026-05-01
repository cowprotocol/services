use {serde::Deserialize, serde_with::serde_as, url::Url};

/// Default number of winning solutions selected by the local arbitrator.
///
/// Acts as a *protocol parameter* once pod takes real auction traffic —
/// changing it has the same blast radius as a parameter hardfork — so it is
/// surfaced as config from the first version that ships.
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
