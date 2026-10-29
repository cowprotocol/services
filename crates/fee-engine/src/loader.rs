//! A [`RuleSet`] from a TOML rules file.

use {
    crate::{CompileError, RuleDefinition, RuleSet, SampleOrderError, fee},
    configs::{autopilot::fee_policy::default_max_partner_fee, fee_factor::FeeFactor},
    rust_decimal::{Decimal, prelude::FromPrimitive},
    serde::{Deserialize, Deserializer, de::Error as _},
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
    partner_fees: Option<PartnerFees>,
}

/// `[partner_fees]`: how the engine caps an order's partner fees.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PartnerFees {
    /// Percent of the volume all partner fees may take together.
    #[serde(deserialize_with = "percent_of_volume")]
    max_volume: FeeFactor,
}

/// A percent of the volume on the fee grid, like a `surplus` `max_volume`.
fn percent_of_volume<'de, D: Deserializer<'de>>(deserializer: D) -> Result<FeeFactor, D::Error> {
    let percent = f64::deserialize(deserializer)?;
    Decimal::from_f64(percent)
        .ok_or(fee::OUT_OF_RANGE)
        .and_then(|number| fee::factor_on_grid(number, fee::PERCENT))
        .map_err(|error| D::Error::custom(format!("max_volume = {percent} {error}")))
}

/// Parses `source`, compiles the rules and validates them on a sample
/// order. Any failure refuses the whole file.
pub fn load_rules(source: &str) -> Result<RuleSet, LoadError> {
    let file: RulesFile = toml::from_str(source)?;
    // Without `[partner_fees]`, the autopilot's default cap.
    let max_partner_fee = file
        .partner_fees
        .map_or_else(default_max_partner_fee, |table| table.max_volume);
    let rules = RuleSet::new(file.rules, file.lists, max_partner_fee)?;
    rules
        .validate_on_sample_order()
        .map_err(LoadError::SampleOrder)?;
    Ok(rules)
}
