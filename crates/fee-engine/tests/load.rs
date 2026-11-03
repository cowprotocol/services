//! The rules file loader through its public API only.

use {
    alloy_primitives::{Address, U256, address},
    chrono::Utc,
    fee_engine::{
        CompileError,
        Fee,
        Field,
        LoadError,
        Match,
        RuleSet,
        RulesToml,
        load_rules_from_str,
        load_rules_toml,
    },
    model::order::{Order, OrderData},
};

/// The mainnet policy as rules, with short token lists.
const RULES: &str = include_str!("mainnet.toml");

const USDC: Address = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
const USDT: Address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");

fn order(sell: Address, buy: Address) -> Order {
    Order {
        data: OrderData {
            sell_token: sell,
            buy_token: buy,
            sell_amount: U256::from(1_000_000u64),
            buy_amount: U256::from(900_000u64),
            ..Default::default()
        },
        ..Default::default()
    }
}

/// One `[[rule]]` table.
fn rule(name: &str, when: &str, fee: &str) -> String {
    format!("[[rule]]\nname = '{name}'\nwhen = '{when}'\nfee = '{fee}'\n")
}

#[test]
fn mainnet_sample_loads_in_file_order() {
    let rules = load_rules_from_str(RULES).unwrap();
    let names: Vec<_> = rules.rules().iter().map(|rule| rule.name()).collect();
    assert_eq!(
        names,
        [
            "JIT makers pay no protocol fee",
            "Same token (incl. WETH -> ETH): no volume fee",
            "Stablecoin bucket: 0.3 bps",
            "ETH-correlated bucket: 0.3 bps",
            "Default: 2 bps",
        ]
    );
}

#[test]
fn mainnet_sample_lists_reach_the_rules() {
    let rules = load_rules_from_str(RULES).unwrap();
    let evaluator = rules.evaluator(Utc::now());

    let evaluation = evaluator.evaluate(&order(USDC, USDT));
    assert!(evaluation.skipped.is_empty());
    let Match { rule, fees } = evaluation.matched.first().unwrap();
    assert_eq!(rule.name(), "Stablecoin bucket: 0.3 bps");
    // Surplus, not price improvement: `order.class` is always "limit".
    assert!(matches!(
        fees[..],
        [Fee::Surplus { .. }, Fee::Volume { factor }] if factor.to_high_precision() == 30
    ));

    let evaluation = evaluator.evaluate(&order(WETH, USDC));
    let Match { rule, .. } = evaluation.matched.first().unwrap();
    assert_eq!(rule.name(), "Default: 2 bps");
}

#[test]
fn empty_file_has_no_rules() {
    assert!(load_rules_from_str("").unwrap().rules().is_empty());
}

#[test]
fn loads_the_tables_of_a_host_config() {
    let mut config: toml::Table = toml::from_str(RULES).unwrap();
    let tables = RulesToml {
        lists: config.remove("lists"),
        rules: config.remove("rule"),
    };
    let names = |rules: RuleSet| -> Vec<String> {
        rules
            .rules()
            .iter()
            .map(|rule| rule.name().to_owned())
            .collect()
    };
    assert_eq!(
        names(load_rules_toml(tables.clone()).unwrap()),
        names(load_rules_from_str(RULES).unwrap())
    );

    assert!(
        load_rules_toml(RulesToml::default())
            .unwrap()
            .rules()
            .is_empty()
    );
    let lists_only = RulesToml {
        rules: None,
        ..tables
    };
    assert!(load_rules_toml(lists_only).unwrap().rules().is_empty());
}

#[test]
fn refuses_bad_host_config_tables() {
    let error = |source: &str| {
        let mut config: toml::Table = toml::from_str(source).unwrap();
        load_rules_toml(RulesToml {
            lists: config.remove("lists"),
            rules: config.remove("rule"),
        })
        .unwrap_err()
    };
    let fine = rule("x", "true", "no_fee()");

    assert!(matches!(error("lists = []"), LoadError::Toml(_)));
    assert!(matches!(error("rule = 1"), LoadError::Toml(_)));
    assert!(matches!(
        error(&(fine.clone() + "[[rule]]\nname = 'y'\nwhen = 'true'\n")),
        LoadError::MalformedRule { number: 2, .. }
    ));
    assert!(matches!(
        error(&(fine.clone() + &rule("y", "true", "no_fee()") + "extra = 1\n")),
        LoadError::MalformedRule { number: 2, .. }
    ));
    for empty in ["", " "] {
        for definition in [
            rule(empty, "true", "no_fee()"),
            rule("y", empty, "no_fee()"),
            rule("y", "true", empty),
        ] {
            assert!(matches!(
                error(&(fine.clone() + &definition)),
                LoadError::MalformedRule { number: 2, .. }
            ));
        }
    }
    assert!(matches!(
        error(&rule("x", "order.sellToken in", "no_fee()")),
        LoadError::Rules(CompileError::Expression {
            field: Field::When,
            ..
        })
    ));
    assert!(matches!(
        error(&(fine.clone() + &fine)),
        LoadError::Rules(CompileError::DuplicateRule { .. })
    ));
    assert!(matches!(
        error(&rule("x", "false", "volume(0.001)")),
        LoadError::SampleOrder(_)
    ));
    assert_eq!(
        error(&(fine.clone() + "[[rule]]\nname = 'y'\nwhen = 'true'\n"))
            .to_string()
            .trim_end(),
        "rule number 2 is malformed: missing field `fee`"
    );
}

#[test]
fn refuses_bad_files() {
    let error = |source: &str| load_rules_from_str(source).unwrap_err();

    assert!(matches!(error("[lists"), LoadError::Toml(_)));
    assert!(matches!(error("[caps]\nvolume = 1"), LoadError::Toml(_)));
    assert!(matches!(
        error(&(rule("x", "true", "no_fee()") + "extra = 1\n")),
        LoadError::Toml(_)
    ));
    assert!(matches!(
        error("[[rule]]\nname = 'x'\nwhen = 'true'\n"),
        LoadError::Toml(_)
    ));
    assert!(matches!(
        error(&rule("x", "order.sellToken in", "no_fee()")),
        LoadError::Rules(CompileError::Expression {
            field: Field::When,
            ..
        })
    ));
    assert!(matches!(
        error(&(rule("x", "true", "no_fee()") + &rule("x", "true", "no_fee()"))),
        LoadError::Rules(CompileError::DuplicateRule { .. })
    ));
    for empty in ["", " "] {
        for definition in [
            rule(empty, "true", "no_fee()"),
            rule("x", empty, "no_fee()"),
            rule("x", "true", empty),
        ] {
            assert!(matches!(error(&definition), LoadError::Toml(_)));
        }
    }
    assert!(matches!(
        error("[lists]\norder = []"),
        LoadError::Rules(CompileError::ReservedList { .. })
    ));
    let sample_order_error =
        error(&(rule("x", "false", "volume(0.001)") + &rule("y", "order.sellAmount", "no_fee()")));
    assert_eq!(
        sample_order_error.to_string(),
        "rule `x`: `fee` `volume` argument `bps` = 0.001 is finer than the fee precision (1e-6 of \
         the volume) on a sample order; rule `y`: `when` returned float, not a bool on a sample \
         order"
    );
    let LoadError::SampleOrder(errors) = sample_order_error else {
        panic!("expected a sample order error");
    };
    let failed: Vec<_> = errors
        .iter()
        .map(|error| (error.rule.as_str(), error.field))
        .collect();
    assert_eq!(failed, [("x", Field::Fee), ("y", Field::When)]);
}
