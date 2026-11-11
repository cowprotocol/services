//! A [`RuleSet`] from a TOML rules file or from a [`RuleSetDefinition`].

use crate::{CompileError, RuleSet, RuleSetDefinition, SampleOrderError};

/// Why a rules file was refused.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("rules are malformed: {0}")]
    Toml(#[from] toml::de::Error),
    #[error(transparent)]
    Rules(#[from] CompileError),
    #[error("{}", join_sample_order_errors(.0))]
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

/// Parses a whole rules file, then loads it.
pub fn load_rules_from_str(source: &str) -> Result<RuleSet, LoadError> {
    load_rules(toml::from_str(source)?)
}

/// Compiles the rules and validates them on a sample order. Any failure
/// refuses the whole set.
pub fn load_rules(definition: RuleSetDefinition) -> Result<RuleSet, LoadError> {
    let rules = RuleSet::new(definition)?;
    rules
        .validate_on_sample_order()
        .map_err(LoadError::SampleOrder)?;
    Ok(rules)
}
