//! Running a [`RuleSet`] against orders.

use {
    crate::{
        fee::{self, Fee},
        order_view::{self, Order},
        rules::{Field, Rule, RuleSet},
    },
    cel::{Context, Value, objects::ValueType},
    std::fmt,
};

/// What the rules said about one order. The evaluator only matches: the
/// caller picks which matches to act on (e.g. the first) and how to report
/// `skipped`.
#[derive(Debug)]
pub struct Evaluation<'r> {
    /// Every rule whose `when` held, in rule order, with its fees. Empty means
    /// no fee applies (check `skipped` to ensure no fees were ignored).
    pub matched: Vec<Match<'r>>,
    /// Rules that failed to evaluate, in rule order.
    pub skipped: Vec<Skipped<'r>>,
}

/// A rule whose `when` held for an order, and its fees.
#[derive(Debug)]
pub struct Match<'r> {
    pub rule: &'r Rule,
    pub fees: Vec<Fee>,
}

/// A rule the evaluator passed over, and the expression that failed.
#[derive(Debug)]
pub struct Skipped<'r> {
    pub rule: &'r Rule,
    pub field: Field,
    pub error: RuleError,
}

impl fmt::Display for Skipped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "rule `{}`: `{}` {}",
            self.rule.name(),
            self.field,
            self.error
        )
    }
}

/// Why an expression yielded no usable value for an order.
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    #[error("failed: {0}")]
    Execution(#[from] cel::ExecutionError),
    #[error("returned {0}, not a bool")]
    NotABool(ValueType),
    #[error("{0}")]
    NotAFee(String),
}

/// A [`RuleSet`] with `now` and its lists bound, ready to evaluate orders.
pub struct Evaluator<'r> {
    rules: &'r RuleSet,
    context: Context<'static, 'static>,
}

impl<'r> Evaluator<'r> {
    pub fn new(rules: &'r RuleSet, context: Context<'static, 'static>) -> Self {
        Self { rules, context }
    }

    /// Runs every rule against `order`. A rule that fails to evaluate is
    /// skipped and reported; it never blocks the others.
    pub fn evaluate(&self, order: Order) -> Evaluation<'r> {
        let context = self.scope(order);
        let mut evaluation = Evaluation {
            matched: Vec::new(),
            skipped: Vec::new(),
        };
        for rule in self.rules.rules() {
            match Self::evaluate_rule(rule, &context) {
                Ok(Some(fees)) => evaluation.matched.push(Match { rule, fees }),
                Ok(None) => {}
                Err((field, error)) => evaluation.skipped.push(Skipped { rule, field, error }),
            }
        }
        evaluation
    }

    /// Runs both expressions of every rule against `order`, whatever `when`
    /// says, and reports every one that fails, in rule order.
    pub(crate) fn evaluate_all_expressions(&self, order: Order) -> Vec<Skipped<'r>> {
        let context = self.scope(order);
        let mut skipped = Vec::new();
        for rule in self.rules.rules() {
            let errors = [
                Self::matched(rule, &context).err(),
                Self::fees(rule, &context).err(),
            ];
            for (field, error) in errors.into_iter().flatten() {
                skipped.push(Skipped { rule, field, error });
            }
        }
        skipped
    }

    fn scope(&self, order: Order) -> Context<'_, '_> {
        let mut context = self.context.new_inner_scope();
        context
            .add_variable("order", order_view::order_view(order))
            .expect("JSON converts to CEL values");
        context
    }

    /// `Ok(None)` when `when` is false, `Ok(Some(fees))` when it is true.
    fn evaluate_rule(
        rule: &Rule,
        context: &Context<'_, '_>,
    ) -> Result<Option<Vec<Fee>>, (Field, RuleError)> {
        if !Self::matched(rule, context)? {
            return Ok(None);
        }
        Self::fees(rule, context).map(Some)
    }

    fn matched(rule: &Rule, context: &Context<'_, '_>) -> Result<bool, (Field, RuleError)> {
        match rule.when.execute(context) {
            Ok(Value::Bool(matched)) => Ok(matched),
            Ok(other) => Err((Field::When, RuleError::NotABool(other.type_of()))),
            Err(err) => Err((Field::When, err.into())),
        }
    }

    fn fees(rule: &Rule, context: &Context<'_, '_>) -> Result<Vec<Fee>, (Field, RuleError)> {
        let value = rule
            .fee
            .execute(context)
            .map_err(|err| (Field::Fee, err.into()))?;
        fee::fees_from_value(&value).map_err(|message| (Field::Fee, RuleError::NotAFee(message)))
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::rules::RuleDefinition,
        alloy_primitives::{Address, U256, address},
        chrono::{DateTime, TimeZone, Utc},
        configs::fee_factor::FeeFactor,
        model::order::{
            EthflowData,
            OnchainOrderData,
            OnchainOrderPlacementError,
            OrderData,
            OrderMetadata,
        },
        std::collections::BTreeMap,
    };

    const USDC: Address = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
    const USDT: Address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
    const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
    const MAKER: Address = address!("0x1111111111111111111111111111111111111111");

    fn rule(name: &str, when: &str, fee: &str) -> RuleDefinition {
        RuleDefinition {
            name: name.to_owned(),
            when: when.to_owned(),
            fee: fee.to_owned(),
        }
    }

    fn sample_rules() -> Vec<RuleDefinition> {
        vec![
            rule("JIT makers", "order.owner in jit_makers", "no_fee()"),
            rule(
                "Stablecoin launch month",
                r#"order.sellToken in stablecoins && order.buyToken in stablecoins && now < timestamp("2026-12-01T00:00:00Z")"#,
                "volume(1)",
            ),
            rule(
                "Stablecoin swaps",
                "order.sellToken in stablecoins && order.buyToken in stablecoins",
                "volume(10)",
            ),
            rule("Default", "true", "surplus(50, 1)"),
        ]
    }

    fn sample_lists() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([
            (
                "stablecoins".to_owned(),
                vec![USDC.to_string(), USDT.to_string()],
            ),
            ("jit_makers".to_owned(), vec![MAKER.to_string()]),
        ])
    }

    fn rule_set(definitions: Vec<RuleDefinition>) -> RuleSet {
        RuleSet::new(definitions, sample_lists()).unwrap()
    }

    fn order(sell: Address, buy: Address) -> model::order::Order {
        model::order::Order {
            metadata: OrderMetadata {
                owner: address!("0x2222222222222222222222222222222222222222"),
                ..Default::default()
            },
            data: OrderData {
                sell_token: sell,
                buy_token: buy,
                sell_amount: U256::from(1_000_000u64),
                buy_amount: U256::from(900_000u64),
                valid_to: 1_800_000_000,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn after_launch() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap()
    }

    fn names<'r>(evaluation: &Evaluation<'r>) -> Vec<&'r str> {
        evaluation.matched.iter().map(|m| m.rule.name()).collect()
    }

    fn matched<'r>(evaluation: &Evaluation<'r>) -> (&'r str, Vec<Fee>) {
        let Match { rule, fees } = evaluation.matched.first().expect("expected a match");
        (rule.name(), fees.clone())
    }

    fn volume(bps: f64) -> Fee {
        Fee::Volume {
            factor: FeeFactor::try_from(bps / 10_000.).unwrap(),
        }
    }

    #[test]
    fn reports_every_matching_rule_in_order() {
        let rules = rule_set(sample_rules());
        let evaluator = rules.evaluator(after_launch());

        let evaluation = evaluator.evaluate(Order {
            order: &order(USDC, USDT),
            in_market: false,
        });
        assert_eq!(names(&evaluation), ["Stablecoin swaps", "Default"]);
        let (rule, fees) = matched(&evaluation);
        assert_eq!(rule, "Stablecoin swaps");
        assert_eq!(fees, vec![volume(10.)]);

        let evaluation = evaluator.evaluate(Order {
            order: &order(WETH, USDT),
            in_market: false,
        });
        assert_eq!(names(&evaluation), ["Default"]);
        let (rule, fees) = matched(&evaluation);
        assert_eq!(rule, "Default");
        assert_eq!(
            fees,
            vec![Fee::Surplus {
                factor: FeeFactor::try_from(0.5).unwrap(),
                max_volume_factor: FeeFactor::try_from(0.01).unwrap(),
            }]
        );

        let mut jit = order(USDC, USDT);
        jit.metadata.owner = MAKER;
        let evaluation = evaluator.evaluate(Order {
            order: &jit,
            in_market: false,
        });
        assert_eq!(
            names(&evaluation),
            ["JIT makers", "Stablecoin swaps", "Default"]
        );
        let (rule, fees) = matched(&evaluation);
        assert_eq!(rule, "JIT makers");
        assert_eq!(fees, vec![]);
    }

    #[test]
    fn evaluations_do_not_leak_between_orders() {
        let rules = rule_set(vec![
            rule("JIT makers", "order.owner in jit_makers", "no_fee()"),
            rule("Large", "order.sellAmount >= 1e6", "volume(1)"),
            rule("Small", "true", "volume(2)"),
        ]);
        let evaluator = rules.evaluator(after_launch());

        let large = order(WETH, USDT);
        let mut small = order(WETH, USDT);
        small.data.sell_amount = U256::from(1u64);
        let mut jit = order(WETH, USDT);
        jit.metadata.owner = MAKER;

        let sequence: Vec<_> = [&large, &small, &jit, &small, &large, &jit]
            .into_iter()
            .map(|order| {
                names(&evaluator.evaluate(Order {
                    order,
                    in_market: false,
                }))
            })
            .collect();
        assert_eq!(
            sequence,
            [
                vec!["Large", "Small"],
                vec!["Small"],
                vec!["JIT makers", "Large", "Small"],
                vec!["Small"],
                vec!["Large", "Small"],
                vec!["JIT makers", "Large", "Small"],
            ]
        );
    }

    #[test]
    fn now_is_the_evaluator_time() {
        let rules = rule_set(sample_rules());
        let launch = rules.evaluator(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap());
        let (rule, fees) = matched(&launch.evaluate(Order {
            order: &order(USDC, USDT),
            in_market: false,
        }));
        assert_eq!(rule, "Stablecoin launch month");
        assert_eq!(fees, vec![volume(1.)]);
    }

    #[test]
    fn list_members_match_regardless_of_case() {
        let lists = BTreeMap::from([(
            "stablecoins".to_owned(),
            vec![USDC.to_string().to_uppercase().replace("0X", "0x")],
        )]);
        let rules = RuleSet::new(
            vec![rule(
                "Stable sells",
                "order.sellToken in stablecoins",
                "volume(1)",
            )],
            lists,
        )
        .unwrap();
        let evaluator = rules.evaluator(after_launch());
        let (rule, _) = matched(&evaluator.evaluate(Order {
            order: &order(USDC, WETH),
            in_market: false,
        }));
        assert_eq!(rule, "Stable sells");
    }

    #[test]
    fn failing_rules_are_skipped() {
        let rules = rule_set(vec![
            rule("Broken when", "order.quote.sellAmount == 1", "volume(1)"),
            rule("Not a bool", "order.sellAmount", "volume(1)"),
            rule("Broken fee", "true", r#"volume("ten")"#),
            rule("Default", "true", "volume(2)"),
            rule("Broken below", "true", "volume(0.001)"),
        ]);
        let evaluator = rules.evaluator(after_launch());
        let evaluation = evaluator.evaluate(Order {
            order: &order(WETH, USDT),
            in_market: false,
        });
        assert_eq!(names(&evaluation), ["Default"]);
        let (_, fees) = matched(&evaluation);
        assert_eq!(fees, vec![volume(2.)]);

        let skipped: Vec<_> = evaluation
            .skipped
            .iter()
            .map(|skipped| (skipped.rule.name(), skipped.field, skipped.to_string()))
            .collect();
        assert!(matches!(
            skipped[..],
            [
                ("Broken when", Field::When, _),
                ("Not a bool", Field::When, _),
                ("Broken fee", Field::Fee, _),
                ("Broken below", Field::Fee, _),
            ]
        ));
        assert!(matches!(
            evaluation.skipped[0].error,
            RuleError::Execution(cel::ExecutionError::NoSuchKey(_))
        ));
        assert!(matches!(
            evaluation.skipped[1].error,
            RuleError::NotABool(ValueType::Float)
        ));
        assert_eq!(
            skipped[2].2,
            "rule `Broken fee`: `fee` `volume` argument `bps` is string instead of a number"
        );
    }

    #[test]
    fn math_extension_is_available() {
        let rules = rule_set(vec![rule(
            "Scaled",
            "true",
            "volume(math.greatest(1.0, math.least(10.0, order.sellAmount / 1e5)))",
        )]);
        let evaluator = rules.evaluator(after_launch());
        let (_, fees) = matched(&evaluator.evaluate(Order {
            order: &order(WETH, USDT),
            in_market: false,
        }));
        assert_eq!(fees, vec![volume(10.)]);
    }

    #[test]
    fn strings_extension_lowercases_literals() {
        let rules = rule_set(vec![rule(
            "Checksummed literal",
            "order.sellToken == '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48'.lowerAscii()",
            "volume(1)",
        )]);
        let evaluator = rules.evaluator(after_launch());
        let (rule, _) = matched(&evaluator.evaluate(Order {
            order: &order(USDC, WETH),
            in_market: false,
        }));
        assert_eq!(rule, "Checksummed literal");
    }

    #[test]
    fn absent_optional_fields_are_tested_with_has() {
        let rules = rule_set(vec![
            rule("Quoted", "has(order.quote)", "volume(1)"),
            rule("Fast path", "order.fastPath", "volume(2)"),
            rule("Default", "true", "volume(3)"),
        ]);
        let evaluator = rules.evaluator(after_launch());

        let evaluation = evaluator.evaluate(Order {
            order: &order(WETH, USDT),
            in_market: false,
        });
        assert_eq!(names(&evaluation), ["Default"]);
        assert!(evaluation.skipped.is_empty());

        let mut quoted = order(WETH, USDT);
        quoted.metadata.quote = Some(Default::default());
        quoted.metadata.fast_path = true;
        let evaluation = evaluator.evaluate(Order {
            order: &quoted,
            in_market: false,
        });
        assert_eq!(names(&evaluation), ["Quoted", "Fast path", "Default"]);
    }

    #[test]
    fn absent_nested_fields_are_tested_with_has() {
        let rules = rule_set(vec![
            rule(
                "Refunded",
                "has(order.ethflowData.refundTxHash)",
                "volume(1)",
            ),
            rule(
                "Placement error",
                "has(order.onchainOrderData.placementError)",
                "volume(2)",
            ),
            rule("Default", "true", "volume(3)"),
        ]);
        let evaluator = rules.evaluator(after_launch());

        let mut onchain = order(WETH, USDT);
        onchain.metadata.ethflow_data = Some(Default::default());
        onchain.metadata.onchain_order_data = Some(Default::default());
        let evaluation = evaluator.evaluate(Order {
            order: &onchain,
            in_market: false,
        });
        assert_eq!(names(&evaluation), ["Default"]);
        assert!(evaluation.skipped.is_empty());

        onchain.metadata.ethflow_data = Some(EthflowData {
            refund_tx_hash: Some(Default::default()),
            ..Default::default()
        });
        onchain.metadata.onchain_order_data = Some(OnchainOrderData {
            placement_error: Some(OnchainOrderPlacementError::InvalidQuote),
            ..Default::default()
        });
        let evaluation = evaluator.evaluate(Order {
            order: &onchain,
            in_market: false,
        });
        assert_eq!(
            names(&evaluation),
            ["Refunded", "Placement error", "Default"]
        );
    }

    #[test]
    fn in_market_is_the_one_passed_in() {
        let rules = rule_set(vec![
            rule("Inside", "order.inMarket", "volume(1)"),
            rule("Outside", "!order.inMarket", "volume(2)"),
        ]);
        let evaluator = rules.evaluator(after_launch());
        let order = order(WETH, USDT);
        assert_eq!(
            names(&evaluator.evaluate(Order {
                order: &order,
                in_market: true
            })),
            ["Inside"]
        );
        assert_eq!(
            names(&evaluator.evaluate(Order {
                order: &order,
                in_market: false
            })),
            ["Outside"]
        );
    }

    #[test]
    fn no_rules_means_no_fee() {
        let rules = rule_set(vec![]);
        let evaluator = rules.evaluator(after_launch());
        let evaluation = evaluator.evaluate(Order {
            order: &order(WETH, USDT),
            in_market: false,
        });
        assert!(evaluation.matched.is_empty());
        assert!(evaluation.skipped.is_empty());
    }
}
