//! A [`RuleSet`] from a TOML rules file or from the tables of one.

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

/// The `lists`, `rule` and `partner-fees` tables of a rules file as raw TOML,
/// e.g. from a section of a host config; `None` is an absent table, as in a
/// file. [`load_rules_toml`] checks their shape.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RulesToml {
    pub lists: Option<toml::Value>,
    pub rules: Option<toml::Value>,
    pub partner_fees: Option<toml::Value>,
}

impl RulesToml {
    /// Adds a list whose values the host knows, such as the chain's wrapped
    /// native token. The tables may not define it.
    pub fn add_host_list(
        &mut self,
        name: &str,
        values: impl IntoIterator<Item: ToString>,
    ) -> Result<(), LoadError> {
        let lists = self.lists.get_or_insert_with(|| toml::Table::new().into());
        // Lists that are not a table are left for `load_rules_toml` to refuse.
        if let Some(lists) = lists.as_table_mut() {
            let values: Vec<_> = values.into_iter().map(|value| value.to_string()).collect();
            if lists.insert(name.to_owned(), values.into()).is_some() {
                return Err(CompileError::ReservedList {
                    name: name.to_owned(),
                }
                .into());
            }
        }
        Ok(())
    }
}

/// Parses a whole rules file, then loads it as [`load_rules_toml`] does.
pub fn load_rules_from_str(source: &str) -> Result<RuleSet, LoadError> {
    load_rule_set_definition(toml::from_str(source)?)
}

/// Checks the shape of the tables, compiles the rules and validates them on
/// a sample order. Any failure refuses the whole set.
pub fn load_rules_toml(
    RulesToml {
        lists,
        rules,
        partner_fees,
    }: RulesToml,
) -> Result<RuleSet, LoadError> {
    load_rule_set_definition(RuleSetDefinition {
        lists: lists
            .map(toml::Value::try_into)
            .transpose()?
            .unwrap_or_default(),
        rules: rules.map(rule_definitions).transpose()?.unwrap_or_default(),
        partner_fees: partner_fees
            .map(toml::Value::try_into)
            .transpose()?
            .unwrap_or_default(),
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
