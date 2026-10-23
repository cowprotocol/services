//! The on-disk shape of a rules file.

use {alloy_primitives::Address, serde::Deserialize, std::collections::BTreeMap};

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RulesFile {
    /// Named sets rules test membership against with `in`.
    #[serde(default)]
    pub lists: BTreeMap<String, Vec<String>>,
    /// Partners by app code.
    #[serde(default)]
    pub partners: BTreeMap<String, Partner>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Partner {
    /// Addresses whose attestation proves an order came through the partner.
    pub signers: Vec<Address>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuleDef {
    pub name: String,
    pub when: String,
    pub fee: String,
}
