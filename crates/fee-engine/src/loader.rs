//! A [`RuleSet`] from a TOML rules file.

use {
    crate::{CompileError, RuleDefinition, RuleSet, SampleOrderError},
    serde::Deserialize,
    std::collections::BTreeMap,
};

/// Why a rules file was refused.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("rules file does not parse: {0}")]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    Rules(#[from] CompileError),
    #[error("sample order: {}", join_sample_order_errors(.0))]
    SampleOrder(Vec<SampleOrderError>),
}

/// Every sample order failure on one line.
fn join_sample_order_errors(errors: &[SampleOrderError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// The on-disk shape of a rules file; unknown keys are refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    #[serde(default)]
    lists: BTreeMap<String, Vec<String>>,
    #[serde(default, rename = "rule")]
    rules: Vec<RuleDefinition>,
}

/// Parses `source`, compiles the rules and validates them on a sample
/// order. Any failure refuses the whole file.
pub fn load_rules(source: &str) -> Result<RuleSet, LoadError> {
    let file: RulesFile = toml::from_str(source)?;
    let rules = RuleSet::new(file.rules, file.lists)?;
    rules
        .validate_on_sample_order()
        .map_err(LoadError::SampleOrder)?;
    Ok(rules)
}
