//! Rule definitions and their compilation into a [`RuleSet`].

use {
    crate::{
        AppData,
        Fee,
        FeeFactor,
        OrderView,
        compile::{
            self,
            node::{Set, Val},
        },
        evaluator::{RuleError, Skipped},
        fee,
    },
    alloy_primitives::U256,
    cel::Env,
    chrono::{DateTime, Utc},
    model::order::{EthflowData, OnchainOrderData, Order, OrderData, OrderMetadata},
    rust_decimal::{Decimal, prelude::FromPrimitive},
    serde::{Deserialize, Deserializer, de},
    std::{
        collections::{BTreeMap, HashMap, HashSet},
        fmt,
        sync::Arc,
    },
};

/// A rule as written: a name and two expressions in CEL syntax.
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
/// [partner-fees]
/// max-volume-bps = 100
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
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct RuleSetDefinition {
    #[serde(default)]
    pub lists: BTreeMap<String, Vec<String>>,
    #[serde(default, rename = "rule")]
    pub rules: Vec<RuleDefinition>,
    /// Without the table the engine returns no partner fee.
    pub partner_fees: Option<PartnerFees>,
}

/// How the engine caps an order's partner fees.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartnerFees {
    /// Basis points of the volume all partner fees may take together.
    #[serde(rename = "max-volume-bps", deserialize_with = "bps_of_volume")]
    pub max_volume: FeeFactor,
}

/// Basis points of the volume as a fee factor. Finer than 0.01 bps is refused,
/// not rounded, as for a `volume` argument.
fn bps_of_volume<'de, D: Deserializer<'de>>(deserializer: D) -> Result<FeeFactor, D::Error> {
    let bps = f64::deserialize(deserializer)?;
    Decimal::from_f64(bps)
        .ok_or(fee::FactorError::OutOfRange)
        .and_then(|number| fee::factor_on_grid(number, fee::BPS))
        .map_err(|error| de::Error::custom(format!("max-volume-bps = {bps} {error}")))
}

/// Names the engine binds itself; a list under one of them would be
/// shadowed without any error.
const RESERVED_NAMES: [&str; 1] = ["order"];

/// Why a set of rule definitions was refused.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("rule number {number} has an empty name")]
    EmptyRuleName { number: usize },
    #[error("rule `{name}` is defined twice")]
    DuplicateRule { name: String },
    #[error("list `{name}` takes a name the engine binds itself")]
    ReservedList { name: String },
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
    /// Every optional field present, every timestamp `now`, amounts 1.
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
    when_source: String,
    fee_source: String,
    pub(crate) when: Val<bool>,
    pub(crate) fee: Val<Vec<Fee>>,
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
        lists: &HashMap<String, Arc<Set>>,
        RuleDefinition { name, when, fee }: RuleDefinition,
    ) -> Result<Self, CompileError> {
        let error = |field| {
            let rule = name.clone();
            move |message| CompileError::Expression {
                rule,
                field,
                message,
            }
        };
        let when_compiled = compile::expression(env, &when, lists)
            .and_then(compile::node::Node::into_bool)
            .map_err(error(Field::When))?;
        let fee_compiled = compile::expression(env, &fee, lists)
            .and_then(compile::node::Node::into_fees)
            .map_err(error(Field::Fee))?;
        Ok(Self {
            name,
            when_source: when,
            fee_source: fee,
            when: when_compiled,
            fee: fee_compiled,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The condition as written.
    pub fn when(&self) -> &str {
        &self.when_source
    }

    /// The fee expression as written.
    pub fn fee(&self) -> &str {
        &self.fee_source
    }
}

/// A checked, immutable set of rules.
#[derive(Debug)]
pub struct RuleSet {
    rules: Vec<Rule>,
    max_partner_fee: Option<FeeFactor>,
}

impl RuleSet {
    /// Compiles the rules in the order they are tried. Each list becomes a
    /// set under its name. `partner_fees` caps an order's partner fees
    /// together; without it there are none. Any failure refuses the whole set.
    pub fn new(
        RuleSetDefinition {
            lists,
            rules: definitions,
            partner_fees,
        }: RuleSetDefinition,
    ) -> Result<Self, CompileError> {
        if let Some(name) = lists
            .keys()
            .find(|name| RESERVED_NAMES.contains(&name.as_str()))
        {
            return Err(CompileError::ReservedList { name: name.clone() });
        }
        let lists: HashMap<String, Arc<Set>> = lists
            .iter()
            .map(|(name, members)| {
                let set = Set::new(members.iter().map(String::as_str));
                (name.clone(), Arc::new(set))
            })
            .collect();
        let env = Env::stdlib();
        let mut rules = Vec::with_capacity(definitions.len());
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
            rules.push(Rule::compile(&env, &lists, definition)?);
        }
        Ok(Self {
            rules,
            max_partner_fee: partner_fees.map(|fees| fees.max_volume),
        })
    }

    /// Runs every rule against two sample orders and reports each expression
    /// that fails: every expression on the full sample, then the rules a real
    /// order would skip on the bare one. See the README.
    pub fn validate_on_sample_order(&self) -> Result<(), Vec<SampleOrderError>> {
        let now = Utc::now();
        let error = |sample| {
            move |skipped: Skipped<'_>| SampleOrderError {
                rule: skipped.rule.name().to_owned(),
                field: skipped.field,
                error: skipped.error,
                sample,
            }
        };
        let mut errors: Vec<_> = self
            .evaluate_all_expressions(OrderView {
                order: &full_sample_order(now),
                app_data: &sample_app_data(),
                partner_fees: &[],
                in_market: false,
            })
            .into_iter()
            .map(error(Sample::Full))
            .collect();
        let on_bare: Vec<_> = self
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
    /// `None` counts no partner fee.
    pub fn max_partner_fee(&self) -> Option<FeeFactor> {
        self.max_partner_fee
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
/// reach any name the view may hold.
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
        fast_path: true,
        ..order.metadata
    };
    order
}

/// App data with every field set, for the full sample order.
pub(crate) fn sample_app_data() -> AppData {
    AppData {
        app_code: Some("sample".to_owned()),
        flashloan: true,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, alloy_primitives::Address};

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
    fn list_members_are_addresses_in_any_case_or_texts() {
        let lists = BTreeMap::from([(
            "members".to_owned(),
            vec![
                "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_owned(),
                "Acme".to_owned(),
            ],
        )]);
        let rules = RuleSet::new(RuleSetDefinition {
            lists,
            rules: vec![rule(
                "Members",
                "order.sellToken in members && 'Acme' in members && !('acme' in members)",
                "no_fee()",
            )],
            ..Default::default()
        })
        .unwrap();
        let mut order = Order::default();
        order.data.sell_token = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
            .parse()
            .unwrap();
        let evaluation = rules.evaluate(OrderView {
            order: &order,
            app_data: &AppData::default(),
            partner_fees: &[],
            in_market: false,
        });
        assert_eq!(evaluation.matched.len(), 1);
    }

    /// A short list is scanned and a longer one hashed.
    #[test]
    fn lists_of_any_size_find_their_members() {
        for size in [1u8, 4, 5, 40] {
            let members: Vec<_> = (1..=size).map(Address::with_last_byte).collect();
            let rules = RuleSet::new(RuleSetDefinition {
                lists: BTreeMap::from([(
                    "members".to_owned(),
                    members.iter().map(Address::to_string).collect(),
                )]),
                rules: vec![rule("Members", "order.sellToken in members", "no_fee()")],
                ..Default::default()
            })
            .unwrap();
            let matches = |sell_token| {
                let mut order = Order::default();
                order.data.sell_token = sell_token;
                let evaluation = rules.evaluate(OrderView {
                    order: &order,
                    app_data: &AppData::default(),
                    partner_fees: &[],
                    in_market: false,
                });
                !evaluation.matched.is_empty()
            };
            assert!(members.iter().all(|member| matches(*member)), "{size}");
            assert!(!matches(Address::with_last_byte(size + 1)), "{size}");
        }
    }

    #[test]
    fn folds_a_fee_that_reads_nothing_from_the_order() {
        let rules = RuleSet::new(RuleSetDefinition {
            rules: vec![
                rule("constant", "true", "[surplus(50, 98), volume(2)]"),
                rule("reads the order", "true", "volume(order.inMarket ? 1 : 2)"),
            ],
            ..Default::default()
        })
        .unwrap();
        let constant: Vec<_> = rules
            .rules()
            .iter()
            .map(|rule| matches!(rule.fee, Val::Const(_)))
            .collect();
        assert_eq!(constant, [true, false]);

        let evaluation = rules.evaluate(OrderView {
            order: &Order::default(),
            app_data: &AppData::default(),
            partner_fees: &[],
            in_market: false,
        });
        assert_eq!(
            evaluation.matched[0].fees,
            [
                Fee::Surplus {
                    factor: FeeFactor::try_from(0.5).unwrap(),
                    max_volume_factor: FeeFactor::try_from(0.0098).unwrap(),
                },
                Fee::Volume {
                    factor: FeeFactor::try_from(0.0002).unwrap(),
                },
            ]
        );
        assert!(evaluation.skipped.is_empty());
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

    /// Type errors and unknown names refuse the set; no order reaches them.
    #[test]
    fn refuses_type_errors_and_unknown_names() {
        let message = |when: &str, fee: &str| {
            RuleSet::new(RuleSetDefinition {
                rules: vec![rule("x", when, fee)],
                ..Default::default()
            })
            .unwrap_err()
            .to_string()
        };
        for (when, fee, expected) in [
            (
                "order.sellAmount",
                "volume(1)",
                "expected a bool, found a number",
            ),
            (
                "order.sellAmout > 0",
                "volume(1)",
                "`order` has no field `sellAmout`",
            ),
            (
                "unknown_list == 1",
                "volume(1)",
                "unknown name `unknown_list`",
            ),
            (
                "order.sellAmount == 'x'",
                "volume(1)",
                "cannot compare a number with a string",
            ),
            ("true", "volume('ten')", "expected a number, found a string"),
            ("false", "volume(0.001)", "is finer than the fee precision"),
            (
                "true",
                "order.inMarket ? volume(0.001) : volume(1)",
                "is finer than the fee precision",
            ),
            (
                "wrapped_native.exists(t, t in tokens)",
                "volume(1)",
                "macros",
            ),
            (
                "size(order.kind) > 0",
                "volume(1)",
                "unknown function `size`",
            ),
            (
                "order.kind.startsWith('s')",
                "volume(1)",
                "unknown function `startsWith`",
            ),
            (
                "true",
                "volume(math.greatest(1, 2))",
                "unknown function `greatest`",
            ),
            (
                "order.?validFrom.orValue(0) > 0",
                "volume(1)",
                "unknown function `orValue`",
            ),
            (
                "has(order.?ethflowData.userValidTo)",
                "volume(1)",
                "`.?` is not supported",
            ),
            (
                "now > timestamp('2026-12-01T00:00:00Z')",
                "volume(1)",
                "unknown name `now`",
            ),
            (
                "order.validTo > timestamp('2026-12-01T00:00:00Z')",
                "volume(1)",
                "unknown function `timestamp`",
            ),
            (
                "true",
                "[volume(1), 'x']",
                "only fees or only string literals",
            ),
        ] {
            let error = message(when, fee);
            assert!(error.contains(expected), "{when} / {fee}: {error}");
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
                "Ethflow",
                "has(order.ethflowData) && order.ethflowData.userValidTo > 0",
                "no_fee()",
            ),
            rule("Fast path", "order.fastPath", "no_fee()"),
            rule(
                "App code",
                "has(order.appData.appCode) && order.appData.appCode == 'sample'",
                "no_fee()",
            ),
            rule(
                "Ratio",
                "order.sellAmount > 0",
                "volume(order.buyAmount / order.sellAmount)",
            ),
            rule(
                "Expiry",
                "order.validTo - 3600 > 0 && order.validTo - order.creationDate < 3600",
                "no_fee()",
            ),
            rule(
                "Valid from",
                "has(order.validFrom) && order.validFrom + 1 > 0",
                "no_fee()",
            ),
            rule("Default", "true", "[surplus(50, 100), volume(0.3)]"),
        ])
        .unwrap();

        let errors = |definitions| validate(definitions).unwrap_err();
        assert!(matches!(
            errors(vec![rule(
                "Underflow",
                "order.sellAmount - 2 > 0",
                "volume(1)"
            )])[..],
            [SampleOrderError {
                ref rule,
                field: Field::When,
                error: RuleError::Arithmetic,
                sample: Sample::Full,
            }] if rule == "Underflow"
        ));
        assert_eq!(
            errors(vec![rule("x", "false", "volume(order.sellAmount / 3.0)")])[0].to_string(),
            "rule `x`: `fee` `volume` argument `bps` = 0.3333333333333333333333333333 is finer \
             than the fee precision (1e-6 of the volume) on a sample order"
        );
    }

    #[test]
    fn reports_fields_a_real_order_may_lack() {
        validate(vec![
            rule(
                "Guarded",
                "has(order.ethflowData) && order.ethflowData.userValidTo != 0",
                "volume(1)",
            ),
            rule(
                "Fee behind its guard",
                "has(order.validFrom)",
                "volume(order.validFrom > 0 ? 1 : 2)",
            ),
            rule(
                "Last field only",
                "has(order.ethflowData.userValidTo)",
                "no_fee()",
            ),
        ])
        .unwrap();

        let errors = validate(vec![
            rule(
                "Unguarded",
                "order.ethflowData.userValidTo != 0",
                "volume(1)",
            ),
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
                ("Unguarded", Field::When, Sample::Bare),
                ("App code", Field::When, Sample::Bare),
            ]
        );
        assert!(
            errors
                .iter()
                .all(|error| matches!(error.error, RuleError::Missing(_)))
        );
        assert_eq!(
            errors[0].to_string(),
            "rule `Unguarded`: `when` the order has no `userValidTo` on a sample order with no \
             optional field"
        );
    }

    #[test]
    fn sample_order_validation_reports_every_failure() {
        let rules = RuleSet::new(RuleSetDefinition {
            rules: vec![
                rule(
                    "Both",
                    "order.sellAmount - 2 > 0",
                    "volume(order.sellAmount / 3.0)",
                ),
                rule("Fine", "true", "volume(1)"),
                rule("Fee", "true", "volume(order.sellAmount / 3.0)"),
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
