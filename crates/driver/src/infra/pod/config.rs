use {serde::Deserialize, serde_with::serde_as, url::Url};

#[serde_as]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    pub endpoint: Url,
    pub auction_contract_address: pod_sdk::alloy_primitives::Address,
}
