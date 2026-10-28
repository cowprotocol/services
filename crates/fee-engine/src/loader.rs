//! A [`RuleSet`] from a TOML rules file.

use {
    crate::{CompileError, PreflightError, RuleDefinition, RuleSet},
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
    #[error("preflight: {}", list(.0))]
    Preflight(Vec<PreflightError>),
}

/// Every preflight failure on one line.
fn list(errors: &[PreflightError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

/// The on-disk shape of a rules file; unknown keys are refused.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    #[serde(default)]
    lists: BTreeMap<String, Vec<String>>,
    #[serde(default, rename = "rule")]
    rules: Vec<RuleDefinition>,
}

/// Parses `source`, compiles the rules and runs the preflight. Any failure
/// refuses the whole file.
pub fn load_rules(source: &str) -> Result<RuleSet, LoadError> {
    let file: RulesFile = toml::from_str(source)?;
    let rules = RuleSet::new(file.rules, file.lists)?;
    rules.preflight().map_err(LoadError::Preflight)?;
    Ok(rules)
}
