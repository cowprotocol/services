//! A [`RuleSet`] from a TOML rules file or from the two tables of one.

use crate::{CompileError, RuleDefinition, RuleSet, RuleSetDefinition, SampleOrderError};

/// Why a rules file was refused.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("rules are malformed: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("rule number {number} is malformed: {error}")]
    MalformedRule {
        number: usize,
        error: toml::de::Error,
    },
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
/// from a section of a host config; `None` is empty, as an absent table is in
/// a file. [`load_rules_toml`] checks their shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RulesToml {
    pub lists: Option<toml::Value>,
    pub rules: Option<toml::Value>,
}

/// Parses a whole rules file, then loads it as [`load_rules_toml`] does.
pub fn load_rules_from_str(source: &str) -> Result<RuleSet, LoadError> {
    load_rule_set_definition(toml::from_str(source)?)
}

/// Checks the shape of both tables, compiles the rules and validates them on
/// a sample order. Any failure refuses the whole set.
pub fn load_rules_toml(RulesToml { lists, rules }: RulesToml) -> Result<RuleSet, LoadError> {
    load_rule_set_definition(RuleSetDefinition {
        lists: lists
            .map(toml::Value::try_into)
            .transpose()?
            .unwrap_or_default(),
        rules: rules.map(rule_definitions).transpose()?.unwrap_or_default(),
    })
}

/// Deserializes each rule on its own on purpose, for better diagnostics: a
/// `toml::Value` has no line to point at, so the error names the rule's
/// number instead.
fn rule_definitions(rules: toml::Value) -> Result<Vec<RuleDefinition>, LoadError> {
    let rules: Vec<toml::Value> = rules.try_into()?;
    rules
        .into_iter()
        .enumerate()
        .map(|(index, rule)| {
            rule.try_into().map_err(|error| LoadError::MalformedRule {
                number: index + 1,
                error,
            })
        })
        .collect()
}

fn load_rule_set_definition(definition: RuleSetDefinition) -> Result<RuleSet, LoadError> {
    let rules = RuleSet::new(definition)?;
    rules
        .validate_on_sample_order()
        .map_err(LoadError::SampleOrder)?;
    Ok(rules)
}
