//! A [`RuleSet`] from a TOML rules file or from the two tables of one.

use crate::{CompileError, RuleSet, RuleSetDefinition, SampleOrderError};

/// Why a rules file was refused.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("rules do not parse: {0}")]
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

/// The `lists` table and the `rule` array of a rules file as raw TOML, e.g.
/// from a section of a host config. [`load_rules_toml`] checks their shape.
#[derive(Debug, Clone, PartialEq)]
pub struct RulesToml {
    pub lists: toml::Value,
    pub rules: toml::Value,
}

impl Default for RulesToml {
    fn default() -> Self {
        Self {
            lists: toml::Value::Table(Default::default()),
            rules: toml::Value::Array(Default::default()),
        }
    }
}

/// Parses a whole rules file, then loads it as [`load_rules_toml`] does.
pub fn load_rules_from_str(source: &str) -> Result<RuleSet, LoadError> {
    load_rule_set_definition(toml::from_str(source)?)
}

/// Checks the shape of both tables, compiles the rules and validates them on
/// a sample order. Any failure refuses the whole set.
pub fn load_rules_toml(RulesToml { lists, rules }: RulesToml) -> Result<RuleSet, LoadError> {
    load_rule_set_definition(RuleSetDefinition {
        lists: lists.try_into()?,
        rules: rules.try_into()?,
    })
}

fn load_rule_set_definition(definition: RuleSetDefinition) -> Result<RuleSet, LoadError> {
    let rules = RuleSet::new(definition)?;
    rules
        .validate_on_sample_order()
        .map_err(LoadError::SampleOrder)?;
    Ok(rules)
}
