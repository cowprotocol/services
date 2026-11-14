use serde::{Deserialize, Serialize};

/// The `[fee-engine]` tables as raw TOML. A host loads them with
/// `fee_engine::load_rules_toml`, which names a malformed rule by its number;
/// a typed section would fail the whole config parse, whose error is hidden.
#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct FeeEngineConfig {
    pub lists: Option<toml::Value>,
    #[serde(rename = "rule")]
    pub rules: Option<toml::Value>,
    pub partner_fees: Option<toml::Value>,
}
