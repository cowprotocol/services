//! Running a [`RuleSet`] against orders.

use {
    crate::{
        fee::{FactorError, Fee, FeeKind},
        order_view::{Input, OrderView},
        partner_fee,
        rules::{Field, Rule, RuleSet},
    },
    chrono::{DateTime, Utc},
    rust_decimal::Decimal,
    std::fmt,
};

/// What the rules said about one order. The evaluator only matches: the
/// caller picks which matches to act on (e.g. the first) and how to report
/// `skipped`.
#[derive(Debug)]
pub struct Evaluation<'r> {
    /// Every rule whose `when` held, in rule order, with its fees. Empty means
    /// no protocol fee applies (check `skipped` to ensure none was ignored).
    pub matched: Vec<Match<'r>>,
    /// Rules that failed to evaluate, in rule order.
    pub skipped: Vec<Skipped<'r>>,
    /// The order's partner fees, capped; none when the rule set has no cap.
    /// They come after the fees of the match the caller picks, and apply when
    /// no rule matched too.
    pub partner_fees: Vec<Fee>,
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

/// Why an expression yielded no value for an order; the rule is skipped.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RuleError {
    #[error("the order has no `{0}`")]
    Missing(&'static str),
    #[error("time arithmetic overflowed")]
    Overflow,
    #[error("arithmetic overflowed or divided by zero")]
    Arithmetic,
    #[error("the number does not fit a {0}")]
    Range(&'static str),
    #[error("`{kind}` argument `{argument}` = {value} {error}")]
    Fee {
        kind: FeeKind,
        argument: &'static str,
        value: Decimal,
        error: FactorError,
    },
}

/// A [`RuleSet`] at a fixed `now`, ready to evaluate orders.
pub struct Evaluator<'r> {
    rules: &'r RuleSet,
    now: DateTime<Utc>,
}

impl<'r> Evaluator<'r> {
    pub(crate) fn new(rules: &'r RuleSet, now: DateTime<Utc>) -> Self {
        Self { rules, now }
    }

    /// Runs every rule against `order`. A rule that fails to evaluate is
    /// skipped and reported; it never blocks the others.
    pub fn evaluate(&self, order: OrderView) -> Evaluation<'r> {
        let input = self.input(order);
        let mut evaluation = Evaluation {
            matched: Vec::new(),
            skipped: Vec::new(),
            partner_fees: self
                .rules
                .max_partner_fee()
                .map(|cap| partner_fee::partner_fees(order.partner_fees, cap))
                .unwrap_or_default(),
        };
        for rule in self.rules.rules() {
            match rule.when.eval(&input) {
                Ok(false) => {}
                Ok(true) => match rule.fee.eval(&input) {
                    Ok(fees) => evaluation.matched.push(Match { rule, fees }),
                    Err(error) => evaluation.skipped.push(Skipped {
                        rule,
                        field: Field::Fee,
                        error,
                    }),
                },
                Err(error) => evaluation.skipped.push(Skipped {
                    rule,
                    field: Field::When,
                    error,
                }),
            }
        }
        evaluation
    }

    /// Runs both expressions of every rule against `order`, whatever `when`
    /// says, and reports every one that fails, in rule order.
    pub(crate) fn evaluate_all_expressions(&self, order: OrderView) -> Vec<Skipped<'r>> {
        let input = self.input(order);
        let mut skipped = Vec::new();
        for rule in self.rules.rules() {
            if let Err(error) = rule.when.eval(&input) {
                skipped.push(Skipped {
                    rule,
                    field: Field::When,
                    error,
                });
            }
            if let Err(error) = rule.fee.eval(&input) {
                skipped.push(Skipped {
                    rule,
                    field: Field::Fee,
                    error,
                });
            }
        }
        skipped
    }

    fn input<'a>(&self, order: OrderView<'a>) -> Input<'a> {
        Input {
            order,
            now: self.now,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{
            AppData,
            FeeFactor,
            PartnerFee,
            rules::{PartnerFees, RuleDefinition, RuleSetDefinition},
        },
        alloy_primitives::{Address, U256, address},
        chrono::{DateTime, TimeZone, Utc},
        model::order::{Order, OrderData, OrderMetadata},
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
            rule("Default", "true", "surplus(50, 100)"),
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
        RuleSet::new(RuleSetDefinition {
            lists: sample_lists(),
            rules: definitions,
            ..Default::default()
        })
        .unwrap()
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

    /// `order` evaluated with no app data and no partner fees.
    fn evaluate<'r>(evaluator: &Evaluator<'r>, order: &Order, in_market: bool) -> Evaluation<'r> {
        evaluator.evaluate(OrderView {
            order,
            app_data: &AppData::default(),
            partner_fees: &[],
            in_market,
        })
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

        let evaluation = evaluate(&evaluator, &order(USDC, USDT), false);
        assert_eq!(names(&evaluation), ["Stablecoin swaps", "Default"]);
        let (rule, fees) = matched(&evaluation);
        assert_eq!(rule, "Stablecoin swaps");
        assert_eq!(fees, vec![volume(10.)]);

        let evaluation = evaluate(&evaluator, &order(WETH, USDT), false);
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
        let evaluation = evaluate(&evaluator, &jit, false);
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
            .map(|order| names(&evaluate(&evaluator, order, false)))
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
        let (rule, fees) = matched(&evaluate(&launch, &order(USDC, USDT), false));
        assert_eq!(rule, "Stablecoin launch month");
        assert_eq!(fees, vec![volume(1.)]);
    }

    #[test]
    fn list_members_match_regardless_of_case() {
        let lists = BTreeMap::from([(
            "stablecoins".to_owned(),
            vec![USDC.to_string().to_uppercase().replace("0X", "0x")],
        )]);
        let rules = RuleSet::new(RuleSetDefinition {
            lists,
            rules: vec![rule(
                "Stable sells",
                "order.sellToken in stablecoins",
                "volume(1)",
            )],
            ..Default::default()
        })
        .unwrap();
        let evaluator = rules.evaluator(after_launch());
        let (rule, _) = matched(&evaluate(&evaluator, &order(USDC, WETH), false));
        assert_eq!(rule, "Stable sells");
    }

    #[test]
    fn failing_rules_are_skipped() {
        let rules = rule_set(vec![
            rule(
                "Broken when",
                "order.ethflowData.userValidTo == 1",
                "volume(1)",
            ),
            rule(
                "Underflow",
                "order.sellAmount - 2u * order.buyAmount > 0u",
                "volume(1)",
            ),
            rule("Broken fee", "true", "volume(order.inMarket ? 1 : 0.001)"),
            rule("Default", "true", "volume(2)"),
            rule("Broken below", "true", "volume(order.sellAmount)"),
        ]);
        let evaluator = rules.evaluator(after_launch());
        let evaluation = evaluate(&evaluator, &order(WETH, USDT), false);
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
                ("Underflow", Field::When, _),
                ("Broken fee", Field::Fee, _),
                ("Broken below", Field::Fee, _),
            ]
        ));
        assert!(matches!(
            evaluation.skipped[0].error,
            RuleError::Missing("userValidTo")
        ));
        assert!(matches!(evaluation.skipped[1].error, RuleError::Arithmetic));
        assert_eq!(
            skipped[2].2,
            "rule `Broken fee`: `fee` `volume` argument `bps` = 0.001 is finer than the fee \
             precision (1e-6 of the volume)"
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
        let (_, fees) = matched(&evaluate(&evaluator, &order(WETH, USDT), false));
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
        let (rule, _) = matched(&evaluate(&evaluator, &order(USDC, WETH), false));
        assert_eq!(rule, "Checksummed literal");
    }

    #[test]
    fn absent_optional_fields_are_tested_with_has() {
        let rules = rule_set(vec![
            rule("Ethflow", "has(order.ethflowData)", "volume(1)"),
            rule("Fast path", "order.fastPath", "volume(2)"),
            rule("Default", "true", "volume(3)"),
        ]);
        let evaluator = rules.evaluator(after_launch());

        let evaluation = evaluate(&evaluator, &order(WETH, USDT), false);
        assert_eq!(names(&evaluation), ["Default"]);
        assert!(evaluation.skipped.is_empty());

        let mut ethflow = order(WETH, USDT);
        ethflow.metadata.ethflow_data = Some(Default::default());
        ethflow.metadata.fast_path = true;
        let evaluation = evaluate(&evaluator, &ethflow, false);
        assert_eq!(names(&evaluation), ["Ethflow", "Fast path", "Default"]);
    }

    #[test]
    fn rules_read_the_app_data() {
        let rules = rule_set(vec![rule(
            "CoW Swap",
            r#"has(order.appData.appCode) && order.appData.appCode == "CoW Swap""#,
            "volume(1)",
        )]);
        let evaluator = rules.evaluator(after_launch());

        let order = order(WETH, USDT);
        let evaluation = evaluate(&evaluator, &order, false);
        assert!(evaluation.matched.is_empty());
        assert!(evaluation.skipped.is_empty());

        let app_data = AppData {
            app_code: Some("CoW Swap".to_owned()),
            ..Default::default()
        };
        let evaluation = evaluator.evaluate(OrderView {
            order: &order,
            app_data: &app_data,
            partner_fees: &[],
            in_market: false,
        });
        assert!(evaluation.skipped.is_empty());
        assert_eq!(names(&evaluation), ["CoW Swap"]);
    }

    #[test]
    fn partner_fees_apply_when_no_rule_matches() {
        let order = order(WETH, USDT);
        let evaluate = |partner_fees| {
            let rules = RuleSet::new(RuleSetDefinition {
                rules: vec![rule("Never", "false", "volume(1)")],
                partner_fees,
                ..Default::default()
            })
            .unwrap();
            let evaluation = rules.evaluator(after_launch()).evaluate(OrderView {
                order: &order,
                app_data: &AppData::default(),
                partner_fees: &[PartnerFee::Volume { bps: 60 }],
                in_market: false,
            });
            assert!(evaluation.matched.is_empty());
            evaluation.partner_fees
        };

        let cap = PartnerFees {
            max_volume: FeeFactor::try_from(0.01).unwrap(),
        };
        assert_eq!(evaluate(Some(cap)), [volume(60.0)]);
        // Without a cap the engine counts no partner fee.
        assert_eq!(evaluate(None), []);
    }

    #[test]
    fn in_market_comes_from_the_order_view() {
        // An order carries no `inMarket`; the view exposes the `in_market` bool
        // the host passes, and a rule reads back exactly that value.
        let rules = rule_set(vec![
            rule("Inside", "order.inMarket", "volume(1)"),
            rule("Outside", "!order.inMarket", "volume(2)"),
        ]);
        let evaluator = rules.evaluator(after_launch());
        let order = order(WETH, USDT);
        assert_eq!(names(&evaluate(&evaluator, &order, true)), ["Inside"]);
        assert_eq!(names(&evaluate(&evaluator, &order, false)), ["Outside"]);
    }

    #[test]
    fn no_rules_means_no_fee() {
        let rules = rule_set(vec![]);
        let evaluator = rules.evaluator(after_launch());
        let evaluation = evaluate(&evaluator, &order(WETH, USDT), false);
        assert!(evaluation.matched.is_empty());
        assert!(evaluation.skipped.is_empty());
    }
}
