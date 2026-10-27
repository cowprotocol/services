//! Running a [`RuleSet`] against orders.

use {
    crate::{
        fee::{self, Fee},
        order_view,
        rules::{Rule, RuleSet},
    },
    cel::{Context, Value},
    model::order::Order,
};

/// The first rule whose `when` held for an order, and its fees.
#[derive(Debug)]
pub struct Match<'r> {
    pub rule: &'r Rule,
    pub fees: Vec<Fee>,
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

    /// Runs the rules top to bottom against `order`. A rule that fails to
    /// evaluate is skipped and logged at debug level, as this runs once per
    /// order per rule. `None` means the order pays no protocol fee.
    pub fn evaluate(&self, order: &Order) -> Option<Match<'r>> {
        let mut context = self.context.new_inner_scope();
        context
            .add_variable("order", order_view::order_view(order))
            .expect("JSON converts to CEL values");

        for rule in self.rules.rules() {
            let matched = match rule.when.execute(&context) {
                Ok(Value::Bool(matched)) => matched,
                Ok(other) => {
                    tracing::debug!(
                        rule = %rule.name(),
                        returned = %other.type_of(),
                        "fee rule `when` is not a bool, skipping rule"
                    );
                    false
                }
                Err(err) => {
                    tracing::debug!(rule = %rule.name(), ?err, "fee rule `when` failed, skipping rule");
                    false
                }
            };
            if !matched {
                continue;
            }
            let value = match rule.fee.execute(&context) {
                Ok(value) => value,
                Err(err) => {
                    tracing::debug!(rule = %rule.name(), ?err, "fee rule `fee` failed, skipping rule");
                    continue;
                }
            };
            let fees = match fee::fees_from_value(&value) {
                Ok(fees) => fees,
                Err(err) => {
                    tracing::debug!(rule = %rule.name(), %err, "fee rule `fee` is not a fee, skipping rule");
                    continue;
                }
            };
            return Some(Match { rule, fees });
        }
        None
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
        model::order::{OrderData, OrderMetadata},
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

    fn order(sell: Address, buy: Address) -> Order {
        Order {
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

    fn matched<'r>(evaluation: &Option<Match<'r>>) -> (&'r str, Vec<Fee>) {
        let Match { rule, fees } = evaluation.as_ref().expect("expected a match");
        (rule.name(), fees.clone())
    }

    fn volume(bps: f64) -> Fee {
        Fee::Volume {
            factor: FeeFactor::try_from(bps / 10_000.).unwrap(),
        }
    }

    #[test]
    fn first_matching_rule_wins() {
        let rules = rule_set(sample_rules());
        let evaluator = rules.evaluator(after_launch());

        let (rule, fees) = matched(&evaluator.evaluate(&order(USDC, USDT)));
        assert_eq!(rule, "Stablecoin swaps");
        assert_eq!(fees, vec![volume(10.)]);

        let (rule, fees) = matched(&evaluator.evaluate(&order(WETH, USDT)));
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
        let (rule, fees) = matched(&evaluator.evaluate(&jit));
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

        let outcome = |order: &Order| matched(&evaluator.evaluate(order)).0;
        let sequence: Vec<_> = [&large, &small, &jit, &small, &large, &jit]
            .into_iter()
            .map(outcome)
            .collect();
        assert_eq!(
            sequence,
            [
                "Large",
                "Small",
                "JIT makers",
                "Small",
                "Large",
                "JIT makers"
            ]
        );
    }

    #[test]
    fn now_is_the_evaluator_time() {
        let rules = rule_set(sample_rules());
        let launch = rules.evaluator(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap());
        let (rule, fees) = matched(&launch.evaluate(&order(USDC, USDT)));
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
        let (rule, _) = matched(&evaluator.evaluate(&order(USDC, WETH)));
        assert_eq!(rule, "Stable sells");
    }

    #[test]
    fn failing_rules_are_skipped() {
        let rules = rule_set(vec![
            rule("Broken when", "order.quote.sellAmount == 1", "volume(1)"),
            rule("Not a bool", "order.sellAmount", "volume(1)"),
            rule("Broken fee", "true", r#"volume("ten")"#),
            rule("Default", "true", "volume(2)"),
        ]);
        let evaluator = rules.evaluator(after_launch());
        let (rule, fees) = matched(&evaluator.evaluate(&order(WETH, USDT)));
        assert_eq!(rule, "Default");
        assert_eq!(fees, vec![volume(2.)]);
    }

    #[test]
    fn math_extension_is_available() {
        let rules = rule_set(vec![rule(
            "Scaled",
            "true",
            "volume(math.greatest(1.0, math.least(10.0, order.sellAmount / 1e5)))",
        )]);
        let evaluator = rules.evaluator(after_launch());
        let (_, fees) = matched(&evaluator.evaluate(&order(WETH, USDT)));
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
        let (rule, _) = matched(&evaluator.evaluate(&order(USDC, WETH)));
        assert_eq!(rule, "Checksummed literal");
    }

    #[test]
    fn no_rules_means_no_fee() {
        let rules = rule_set(vec![]);
        let evaluator = rules.evaluator(after_launch());
        assert!(evaluator.evaluate(&order(WETH, USDT)).is_none());
    }
}
