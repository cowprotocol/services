//! Rule definitions and their compilation into a [`RuleSet`].

use {
    crate::{AppData, Evaluator, Flashloan, evaluator::RuleError, fee, order_view},
    alloy_primitives::{Address, U256},
    cel::{Context, Env, Program, Value},
    chrono::{DateTime, Utc},
    model::order::{
        EthflowData,
        OnchainOrderData,
        OnchainOrderPlacementError,
        Order,
        OrderData,
        OrderMetadata,
        OrderQuote,
    },
    serde::Deserialize,
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
    pub name: String,
    pub when: String,
    pub fee: String,
}

/// Names the evaluator binds itself; a list under one of them would be
/// shadowed or overwritten without any error.
const RESERVED_NAMES: [&str; 3] = ["order", "now", "math"];

/// Why a set of rule definitions was refused.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
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

/// One expression that gave no usable value on the sample order.
#[derive(Debug, thiserror::Error)]
#[error("rule `{rule}`: `{field}` {error} on a sample order")]
pub struct SampleOrderError {
    pub rule: String,
    pub field: Field,
    pub error: RuleError,
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
    pub(crate) env: Arc<Env>,
    rules: Vec<Rule>,
    pub(crate) lists: Vec<(String, Value)>,
}

impl fmt::Debug for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuleSet")
            .field("rules", &self.rules)
            .finish_non_exhaustive()
    }
}

impl RuleSet {
    /// Compiles the rules in the order they are tried against the CEL
    /// stdlib plus its math and strings extensions. Each list becomes a CEL
    /// list under its name. Any failure refuses the whole set.
    pub fn new(
        definitions: Vec<RuleDefinition>,
        lists: BTreeMap<String, Vec<String>>,
    ) -> Result<Self, CompileError> {
        let mut env = Env::stdlib().with_optional_support(false);
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
        let mut names = HashSet::new();
        for definition in definitions {
            if !names.insert(definition.name.clone()) {
                return Err(CompileError::DuplicateRule {
                    name: definition.name,
                });
            }
            rules.push(Rule::compile(&env, definition)?);
        }
        Ok(Self {
            env,
            rules,
            lists: list_values(&lists),
        })
    }

    /// Runs both expressions of every rule against a sample order, whatever
    /// `when` says, and reports every one that yields no bool or no fee. This
    /// catches bad fee arguments and misspelt names before any real order.
    pub fn validate_on_sample_order(&self) -> Result<(), Vec<SampleOrderError>> {
        let errors: Vec<_> = self
            .evaluator(Utc::now())
            .evaluate_all_expressions(crate::Order {
                order: &sample_order(),
                app_data: &sample_app_data(),
                in_market: false,
            })
            .into_iter()
            .map(|skipped| SampleOrderError {
                rule: skipped.rule.name().to_owned(),
                field: skipped.field,
                error: skipped.error,
            })
            .collect();
        if errors.is_empty() {
            return Ok(());
        }
        Err(errors)
    }

    /// The rules in the order they are tried.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Binds lists, functions and `now` once; each order evaluated through
    /// the result gets a cheap child scope on top.
    pub fn evaluator(&self, now: DateTime<Utc>) -> Evaluator<'_> {
        let mut context = Context::with_env(self.env.clone());
        fee::register_fee_functions(&mut context);
        for (name, list) in &self.lists {
            context.add_variable_from_value(name.clone(), list.clone());
        }
        context.add_variable_from_value("now", now.fixed_offset());
        Evaluator::new(self, context)
    }
}

/// App data with every field set.
pub(crate) fn sample_app_data() -> AppData {
    AppData {
        app_code: Some("sample".to_owned()),
        signer: Some(Address::ZERO),
        replaced_order: Some([0; 56]),
        flashloan: Some(Flashloan {
            liquidity_provider: Address::ZERO,
            protocol_adapter: Address::ZERO,
            receiver: Address::ZERO,
            token: Address::ZERO,
            amount: U256::ONE,
        }),
    }
}

/// An order with every optional field present, so that a rule can reach any
/// name the view may hold. Amounts and prices are 1, so a ratio of two is
/// defined; `feeAmount` stays 0, as the API refuses any other.
pub(crate) fn sample_order() -> Order {
    Order {
        data: OrderData {
            sell_amount: U256::ONE,
            buy_amount: U256::ONE,
            ..Default::default()
        },
        metadata: OrderMetadata {
            ethflow_data: Some(EthflowData {
                refund_tx_hash: Some(Default::default()),
                ..Default::default()
            }),
            onchain_order_data: Some(OnchainOrderData {
                placement_error: Some(OnchainOrderPlacementError::Other),
                ..Default::default()
            }),
            onchain_user: Some(Default::default()),
            valid_from: Some(Default::default()),
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
            ..Default::default()
        },
        ..Default::default()
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
        let rules = RuleSet::new(
            vec![
                rule("First", "order.kind == 'sell'", "volume(1)"),
                rule("Second", "true", "no_fee()"),
            ],
            BTreeMap::new(),
        )
        .unwrap();
        let names: Vec<_> = rules.rules().iter().map(Rule::name).collect();
        assert_eq!(names, ["First", "Second"]);
        assert_eq!(rules.rules()[0].when(), "order.kind == 'sell'");
        assert_eq!(rules.rules()[1].fee(), "no_fee()");
    }

    #[test]
    fn lowercases_hex_list_members() {
        let lists = BTreeMap::from([(
            "tokens".to_owned(),
            vec!["0xAbC".to_owned(), "Acme".to_owned()],
        )]);
        let rules = RuleSet::new(vec![], lists).unwrap();
        let [(name, members)] = &rules.lists[..] else {
            panic!("expected one list");
        };
        assert_eq!(name, "tokens");
        assert_eq!(
            *members,
            Value::from(vec!["0xabc".to_owned(), "Acme".to_owned()])
        );
    }

    #[test]
    fn refuses_bad_definitions() {
        let error = |definitions| RuleSet::new(definitions, BTreeMap::new()).unwrap_err();

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
    }

    #[test]
    fn refuses_reserved_list_names() {
        for name in RESERVED_NAMES {
            let lists = BTreeMap::from([(name.to_owned(), vec![])]);
            assert!(matches!(
                RuleSet::new(vec![], lists).unwrap_err(),
                CompileError::ReservedList { name: found } if found == name
            ));
        }
    }

    #[test]
    fn validates_every_expression_on_a_sample_order() {
        let validate = |definitions| {
            RuleSet::new(definitions, BTreeMap::new())
                .unwrap()
                .validate_on_sample_order()
        };

        validate(vec![
            rule("Quoted", "order.quote.sellAmount > 0", "volume(1)"),
            rule("Ethflow", "order.ethflowData.userValidTo > 0", "no_fee()"),
            rule("Fast path", "order.fastPath", "no_fee()"),
            rule(
                "Refunded",
                "order.ethflowData.refundTxHash != ''",
                "no_fee()",
            ),
            rule(
                "Placement error",
                "order.onchainOrderData.placementError == 'other'",
                "no_fee()",
            ),
            rule("App code", "order.appData.appCode == 'sample'", "no_fee()"),
            rule(
                "Ratio",
                "order.quote.buyAmount / order.quote.sellAmount > 0.0",
                "volume(math.least(10.0, order.buyAmount / order.sellAmount))",
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
    fn sample_order_validation_reports_every_failure() {
        let rules = RuleSet::new(
            vec![
                rule("Both", "order.sellAmount", "volume(0.001)"),
                rule("Fine", "true", "volume(1)"),
                rule("Fee", "true", "volume('ten')"),
            ],
            BTreeMap::new(),
        )
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
