//! The on-disk shape of a rules file.

use {serde::Deserialize, std::collections::BTreeMap};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RulesFile {
    /// Named sets rules test membership against with `in`.
    #[serde(default)]
    pub lists: BTreeMap<String, Vec<String>>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuleDef {
    pub name: String,
    pub when: String,
    pub fee: String,
}
