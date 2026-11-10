//! Rule definitions and their compilation into a [`RuleSet`].

use {
    crate::{
        AppData,
        Evaluator,
        Flashloan,
        OrderView,
        evaluator::{RuleError, Skipped},
        fee,
        order_view,
    },
    alloy_primitives::{Address, U256},
    cel::{Context, Env, Program, Value},
    chrono::{DateTime, Utc},
    configs::fee_factor::FeeFactor,
    model::order::{EthflowData, OnchainOrderData, Order, OrderData, OrderMetadata, OrderQuote},
    rust_decimal::{Decimal, prelude::FromPrimitive},
    serde::{Deserialize, Deserializer, de},
    std::{
        collections::{BTreeMap, HashSet},
        fmt,
        sync::Arc,
    },
};

/// A rule as written: a name and two CEL expressions.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDefinition {
    #[serde(deserialize_with = "non_empty")]
    pub name: String,
    #[serde(deserialize_with = "non_empty")]
    pub when: String,
    #[serde(deserialize_with = "non_empty")]
    pub fee: String,
}

/// A string with more than whitespace in it.
fn non_empty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        return Err(de::Error::invalid_value(
            de::Unexpected::Str(&value),
            &"a non-empty string",
        ));
    }
    Ok(value)
}

/// A rule set as written: named lists, the rules in the order they are
/// tried and the cap on partner fees.
///
/// ```toml
/// [partner_fees]
/// max_volume_bps = 100
///
/// [lists]
/// stablecoins = ["0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"]
///
/// [[rule]]
/// name = "Stable swaps"
/// when = "order.sellToken in stablecoins && order.buyToken in stablecoins"
/// fee = "volume(0.3)"
/// ```
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleSetDefinition {
    #[serde(default)]
    pub lists: BTreeMap<String, Vec<String>>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<RuleDefinition>,
    #[serde(default)]
    pub partner_fees: PartnerFees,
}

/// How the engine caps an order's partner fees. Without the table,
/// [`DEFAULT_MAX_VOLUME_BPS`].
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartnerFees {
    /// Basis points of the volume all partner fees may take together.
    #[serde(rename = "max_volume_bps", deserialize_with = "bps_of_volume")]
    pub max_volume: FeeFactor,
}

/// The cap on an order's partner fees without a `[partner_fees]` table: 1%
/// of the volume, the autopilot's default.
pub const DEFAULT_MAX_VOLUME_BPS: u64 = 100;

impl Default for PartnerFees {
    fn default() -> Self {
        Self {
            max_volume: FeeFactor::from_high_precision(DEFAULT_MAX_VOLUME_BPS * fee::BPS)
                .expect("100 bps is on the grid and under 100%"),
        }
    }
}

/// Basis points of the volume as a fee factor. Finer than 0.01 bps is refused,
/// not rounded, as for a `volume` argument.
fn bps_of_volume<'de, D: Deserializer<'de>>(deserializer: D) -> Result<FeeFactor, D::Error> {
    let bps = f64::deserialize(deserializer)?;
    Decimal::from_f64(bps)
        .ok_or(fee::FactorError::OutOfRange)
        .and_then(|number| fee::factor_on_grid(number, fee::BPS))
        .map_err(|error| de::Error::custom(format!("max_volume_bps = {bps} {error}")))
}

/// Names the evaluator binds itself; a list under one of them would be
/// shadowed or overwritten without any error.
const RESERVED_NAMES: [&str; 3] = ["order", "now", "math"];

/// Why a set of rule definitions was refused.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("rule number {number} has an empty name")]
    EmptyRuleName { number: usize },
    #[error("rule `{name}` is defined twice")]
    DuplicateRule { name: String },
    #[error("list `{name}` takes a name the engine binds itself")]
    ReservedList { name: String },
    #[error("partner fee cap {cap} is not in [0, 1)")]
    PartnerFeeCap { cap: f64 },
    #[error("rule `{rule}`: `{field}` does not compile: {message}")]
    Expression {
        rule: String,
        field: Field,
        message: String,
    },
}

/// One expression that gave no usable value on a sample order.
#[derive(Debug, thiserror::Error)]
#[error("rule `{rule}`: `{field}` {error} on {sample}")]
pub struct SampleOrderError {
    pub rule: String,
    pub field: Field,
    pub error: RuleError,
    pub sample: Sample,
}

/// The sample order an expression failed on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sample {
    /// Every optional field present, every timestamp `now`, amounts and
    /// prices 1.
    Full,
    /// No optional field present; otherwise the same.
    Bare,
}

impl fmt::Display for Sample {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Sample::Full => "a sample order",
            Sample::Bare => "a sample order with no optional field",
        })
    }
}

/// The two expressions of a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    When,
    Fee,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Field::When => "when",
            Field::Fee => "fee",
        })
    }
}

/// One compiled rule.
pub struct Rule {
    name: String,
    pub(crate) when: Program,
    pub(crate) fee: Program,
}

impl fmt::Debug for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rule")
            .field("name", &self.name)
            .field("when", &self.when())
            .field("fee", &self.fee())
            .finish()
    }
}

impl Rule {
    /// Compiles both expressions of a definition.
    fn compile(
        env: &Env,
        RuleDefinition { name, when, fee }: RuleDefinition,
    ) -> Result<Self, CompileError> {
        let compile = |field, source: &str| {
            env.compile(source).map_err(|err| CompileError::Expression {
                rule: name.clone(),
                field,
                message: err.to_string(),
            })
        };
        let when = compile(Field::When, &when)?;
        let fee = compile(Field::Fee, &fee)?;
        Ok(Self { name, when, fee })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The condition as written.
    pub fn when(&self) -> &str {
        &self.when.source_info().source
    }

    /// The fee expression as written.
    pub fn fee(&self) -> &str {
        &self.fee.source_info().source
    }
}

/// A checked, immutable set of rules.
pub struct RuleSet {
    rules: Vec<Rule>,
    /// The fee functions and the lists, bound once for every evaluation.
    pub(crate) context: Context<'static, 'static>,
    max_partner_fee: FeeFactor,
}

impl fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleSet")
            .field("rules", &self.rules)
            .field("max_partner_fee", &self.max_partner_fee)
            .finish_non_exhaustive()
    }
}

impl RuleSet {
    /// Compiles the rules in the order they are tried against the CEL
    /// stdlib plus its math and strings extensions. Each list becomes a CEL
    /// list under its name. `partner_fees` caps an order's partner fees
    /// together. Any failure refuses the whole set.
    pub fn new(
        RuleSetDefinition {
            lists,
            rules: definitions,
            partner_fees,
        }: RuleSetDefinition,
    ) -> Result<Self, CompileError> {
        let mut env = Env::stdlib();
        cel::extensions::math(&mut env).expect("math names are free in the stdlib");
        cel::extensions::strings(&mut env).expect("strings names are free in the stdlib");
        let env = Arc::new(env);
        let mut rules = Vec::with_capacity(definitions.len());
        if let Some(name) = lists
            .keys()
            .find(|name| RESERVED_NAMES.contains(&name.as_str()))
        {
            return Err(CompileError::ReservedList { name: name.clone() });
        }
        if FeeFactor::try_from(partner_fees.max_volume.get()).is_err() {
            return Err(CompileError::PartnerFeeCap {
                cap: partner_fees.max_volume.get(),
            });
        }
        let mut names = HashSet::new();
        for (index, definition) in definitions.into_iter().enumerate() {
            if definition.name.trim().is_empty() {
                return Err(CompileError::EmptyRuleName { number: index + 1 });
            }
            if !names.insert(definition.name.trim().to_owned()) {
                return Err(CompileError::DuplicateRule {
                    name: definition.name,
                });
            }
            rules.push(Rule::compile(&env, definition)?);
        }
        let mut context = Context::with_env(env);
        fee::register_fee_functions(&mut context);
        for (name, list) in list_values(&lists) {
            context.add_variable_from_value(name, list);
        }
        Ok(Self {
            rules,
            context,
            max_partner_fee: partner_fees.max_volume,
        })
    }

    /// Runs every rule against two sample orders and reports each expression
    /// that yields no bool or no fee: every expression on the full sample,
    /// then the rules a real order would skip on the bare one. See the README.
    pub fn validate_on_sample_order(&self) -> Result<(), Vec<SampleOrderError>> {
        let now = Utc::now();
        let evaluator = self.evaluator(now);
        let error = |sample| {
            move |skipped: Skipped<'_>| SampleOrderError {
                rule: skipped.rule.name().to_owned(),
                field: skipped.field,
                error: skipped.error,
                sample,
            }
        };
        let mut errors: Vec<_> = evaluator
            .evaluate_all_expressions(OrderView {
                order: &full_sample_order(now),
                app_data: &sample_app_data(),
                partner_fees: &[],
                in_market: false,
            })
            .into_iter()
            .map(error(Sample::Full))
            .collect();
        let on_bare: Vec<_> = evaluator
            .evaluate(OrderView {
                order: &bare_sample_order(now),
                app_data: &AppData::default(),
                partner_fees: &[],
                in_market: false,
            })
            .skipped
            .into_iter()
            .filter(|skipped| {
                !errors
                    .iter()
                    .any(|found| found.rule == skipped.rule.name() && found.field == skipped.field)
            })
            .map(error(Sample::Bare))
            .collect();
        errors.extend(on_bare);
        if errors.is_empty() {
            return Ok(());
        }
        Err(errors)
    }

    /// The rules in the order they are tried.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// The cap on an order's partner fees together, as a share of the volume.
    pub fn max_partner_fee(&self) -> FeeFactor {
        self.max_partner_fee
    }

    /// Evaluates orders at `now`. Cheap: lists and functions are bound
    /// once, in [`RuleSet::new`].
    pub fn evaluator(&self, now: DateTime<Utc>) -> Evaluator<'_> {
        Evaluator::new(self, now)
    }
}

/// An order with no optional field present and every timestamp `now`, so
/// that time arithmetic does not underflow. Amounts are 1, so a ratio of two
/// is defined; `feeAmount` stays 0, as the API refuses any other.
pub(crate) fn bare_sample_order(now: DateTime<Utc>) -> Order {
    Order {
        data: OrderData {
            sell_amount: U256::ONE,
            buy_amount: U256::ONE,
            valid_to: now.timestamp() as u32,
            ..Default::default()
        },
        metadata: OrderMetadata {
            creation_date: now,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// The bare sample with every optional field present, so that a rule can
/// reach any name the view may hold. Prices are 1, like the amounts.
pub(crate) fn full_sample_order(now: DateTime<Utc>) -> Order {
    let mut order = bare_sample_order(now);
    order.metadata = OrderMetadata {
        ethflow_data: Some(EthflowData {
            user_valid_to: now.timestamp(),
            ..Default::default()
        }),
        onchain_order_data: Some(OnchainOrderData::default()),
        onchain_user: Some(Default::default()),
        valid_from: Some(now.timestamp() as u32),
        quote: Some(OrderQuote {
            sell_amount: U256::ONE,
            buy_amount: U256::ONE,
            fee_amount: U256::ONE,
            gas_amount: 1.into(),
            gas_price: 1.into(),
            sell_token_price: 1.into(),
            ..Default::default()
        }),
        fast_path: true,
        ..order.metadata
    };
    order
}

/// App data with every field set, for the full sample order.
pub(crate) fn sample_app_data() -> AppData {
    AppData {
        app_code: Some("sample".to_owned()),
        signer: Some(Address::ZERO),
        replaced_order: Some(Default::default()),
        flashloan: Some(Flashloan {
            liquidity_provider: Address::ZERO,
            protocol_adapter: Address::ZERO,
            receiver: Address::ZERO,
            token: Address::ZERO,
            amount: U256::ONE,
        }),
    }
}

/// Each list as the CEL list rules test with `in`, e.g.
/// `order.sellToken in stablecoins`, hex members lowercased like the order
/// view's. `in` scans a list; a member-to-`true` map would make it a key
/// lookup if lists ever grow large.
fn list_values(lists: &BTreeMap<String, Vec<String>>) -> Vec<(String, Value)> {
    lists
        .iter()
        .map(|(name, members)| {
            let members: Vec<String> = members
                .iter()
                .map(|member| order_view::normalize(member))
                .collect();
            (name.clone(), Value::from(members))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(name: &str, when: &str, fee: &str) -> RuleDefinition {
        RuleDefinition {
            name: name.to_owned(),
            when: when.to_owned(),
            fee: fee.to_owned(),
        }
    }

    #[test]
    fn keeps_rule_order_and_sources() {
        let rules = RuleSet::new(RuleSetDefinition {
            rules: vec![
                rule("First", "order.kind == 'sell'", "volume(1)"),
                rule("Second", "true", "no_fee()"),
            ],
            ..Default::default()
        })
        .unwrap();
        let names: Vec<_> = rules.rules().iter().map(Rule::name).collect();
        assert_eq!(names, ["First", "Second"]);
        assert_eq!(rules.rules()[0].when(), "order.kind == 'sell'");
        assert_eq!(rules.rules()[1].fee(), "no_fee()");
    }

    #[test]
    fn rule_set_is_shared_across_threads() {
        fn shared<T: Send + Sync>() {}
        shared::<RuleSet>();
    }

    #[test]
    fn lowercases_hex_list_members() {
        let lists = BTreeMap::from([(
            "tokens".to_owned(),
            vec!["0xAbC".to_owned(), "Acme".to_owned()],
        )]);
        let rules = RuleSet::new(RuleSetDefinition {
            lists,
            rules: vec![rule("Lowercase", "tokens == ['0xabc', 'Acme']", "no_fee()")],
            ..Default::default()
        })
        .unwrap();
        let evaluation = rules.evaluator(Utc::now()).evaluate(OrderView {
            order: &Order::default(),
            app_data: &AppData::default(),
            partner_fees: &[],
            in_market: false,
        });
        assert_eq!(evaluation.matched.len(), 1);
    }

    #[test]
    fn refuses_bad_definitions() {
        let error = |rules| {
            RuleSet::new(RuleSetDefinition {
                rules,
                ..Default::default()
            })
            .unwrap_err()
        };

        assert!(matches!(
            error(vec![rule("x", "order.sellToken in", "volume(1)")]),
            CompileError::Expression {
                field: Field::When,
                ..
            }
        ));
        assert!(matches!(
            error(vec![rule("x", "true", "volume(")]),
            CompileError::Expression {
                field: Field::Fee,
                ..
            }
        ));
        assert!(matches!(
            error(vec![
                rule("x", "true", "volume(1)"),
                rule("x", "true", "volume(1)")
            ]),
            CompileError::DuplicateRule { .. }
        ));
        assert!(matches!(
            error(vec![rule("x", "true", "volume(1)"), rule("x ", "true", "volume(1)")]),
            CompileError::DuplicateRule { ref name } if name == "x "
        ));
        for name in ["", " "] {
            assert!(matches!(
                error(vec![
                    rule("x", "true", "volume(1)"),
                    rule(name, "true", "volume(1)")
                ]),
                CompileError::EmptyRuleName { number: 2 }
            ));
        }
    }

    #[test]
    fn refuses_reserved_list_names() {
        for name in RESERVED_NAMES {
            let lists = BTreeMap::from([(name.to_owned(), vec![])]);
            assert!(matches!(
                RuleSet::new(RuleSetDefinition {
                    lists,
                    ..Default::default()
                })
                .unwrap_err(),
                CompileError::ReservedList { name: found } if found == name
            ));
        }
    }

    fn validate(rules: Vec<RuleDefinition>) -> Result<(), Vec<SampleOrderError>> {
        RuleSet::new(RuleSetDefinition {
            rules,
            ..Default::default()
        })
        .unwrap()
        .validate_on_sample_order()
    }

    #[test]
    fn validates_every_expression_on_a_sample_order() {
        validate(vec![
            rule(
                "Quoted",
                "order.?quote.sellAmount.orValue(0.0) > 0.0",
                "volume(1)",
            ),
            rule(
                "Ethflow",
                "order.?ethflowData.userValidTo.orValue(0u) > 0u",
                "no_fee()",
            ),
            rule("Fast path", "order.fastPath", "no_fee()"),
            rule(
                "App code",
                "order.appData.?appCode.orValue('') == 'sample'",
                "no_fee()",
            ),
            rule(
                "Ratio",
                "order.?quote.optMap(q, q.buyAmount / q.sellAmount).orValue(0.0) > 0.0",
                "volume(math.least(10.0, order.buyAmount / order.sellAmount))",
            ),
            rule(
                "Expiry",
                "order.validTo - 3600u > 0u && now - timestamp(int(order.validTo)) < \
                 duration('1h')",
                "no_fee()",
            ),
            rule(
                "Valid from",
                "order.?validFrom.orValue(0u) + 1u > 0u",
                "no_fee()",
            ),
            rule("Default", "true", "[surplus(50, 1), volume(0.3)]"),
        ])
        .unwrap();

        let errors = |definitions| validate(definitions).unwrap_err();
        assert!(matches!(
            errors(vec![rule("Not a bool", "order.sellAmount", "volume(1)")])[..],
            [SampleOrderError {
                ref rule,
                field: Field::When,
                error: RuleError::NotABool(_),
                sample: Sample::Full,
            }] if rule == "Not a bool"
        ));
        assert!(matches!(
            errors(vec![rule("Misspelt", "order.sellAmout > 0", "volume(1)")])[..],
            [SampleOrderError {
                field: Field::When,
                error: RuleError::Execution(_),
                ..
            }]
        ));
        assert!(matches!(
            errors(vec![rule("Never matches", "false", "volume(0.001)")])[..],
            [SampleOrderError {
                field: Field::Fee,
                error: RuleError::NotAFee(_),
                ..
            }]
        ));
        assert_eq!(
            errors(vec![rule("x", "true", "volume('ten')")])[0].to_string(),
            "rule `x`: `fee` `volume` argument `bps` is string instead of a number on a sample \
             order"
        );
    }

    #[test]
    fn reports_fields_a_real_order_may_lack() {
        validate(vec![
            rule(
                "Guarded",
                "order.?quote.sellAmount.orValue(0.0) > 0.0",
                "volume(1)",
            ),
            rule(
                "Fee behind its guard",
                "order.?quote.hasValue()",
                "volume(order.quote.feeAmount)",
            ),
            rule(
                "Nested",
                "order.?ethflowData.userValidTo.hasValue()",
                "no_fee()",
            ),
        ])
        .unwrap();

        let errors = validate(vec![
            rule("Unguarded", "order.quote.sellAmount > 0", "volume(1)"),
            rule(
                "Last field only",
                "has(order.ethflowData.userValidTo)",
                "no_fee()",
            ),
            rule("Misspelt", "order.sellAmout > 0", "volume(1)"),
            rule("App code", "order.appData.appCode == 'sample'", "no_fee()"),
        ])
        .unwrap_err();
        let reported: Vec<_> = errors
            .iter()
            .map(|error| (error.rule.as_str(), error.field, error.sample))
            .collect();
        assert_eq!(
            reported,
            [
                ("Misspelt", Field::When, Sample::Full),
                ("Unguarded", Field::When, Sample::Bare),
                ("Last field only", Field::When, Sample::Bare),
                ("App code", Field::When, Sample::Bare),
            ]
        );
        assert!(errors.iter().all(|error| matches!(
            error.error,
            RuleError::Execution(cel::ExecutionError::NoSuchKey(_))
        )));
        let message = errors[1].to_string();
        assert!(message.contains("No such key: quote"), "{message}");
        assert!(message.ends_with("on a sample order with no optional field"));
    }

    #[test]
    fn sample_order_validation_reports_every_failure() {
        let rules = RuleSet::new(RuleSetDefinition {
            rules: vec![
                rule("Both", "order.sellAmount", "volume(0.001)"),
                rule("Fine", "true", "volume(1)"),
                rule("Fee", "true", "volume('ten')"),
            ],
            ..Default::default()
        })
        .unwrap();
        let failed: Vec<_> = rules
            .validate_on_sample_order()
            .unwrap_err()
            .iter()
            .map(|error| (error.rule.clone(), error.field))
            .collect();
        assert_eq!(
            failed,
            [
                ("Both".to_owned(), Field::When),
                ("Both".to_owned(), Field::Fee),
                ("Fee".to_owned(), Field::Fee),
            ]
        );
    }
}
