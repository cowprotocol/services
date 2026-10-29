//! The rules file loader through its public API only.

use {
    alloy_primitives::{Address, U256, address},
    chrono::Utc,
    fee_engine::{AppData, CompileError, Fee, Field, LoadError, Match, load_rules},
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

/// `order` with no app data.
fn without_app_data(order: &Order, in_market: bool) -> fee_engine::Order<'_> {
    static NO_APP_DATA: AppData = AppData {
        app_code: None,
        signer: None,
        replaced_order: None,
        flashloan: None,
    };
    fee_engine::Order {
        order,
        app_data: &NO_APP_DATA,
        in_market,
    }
}

/// One `[[rule]]` table.
fn rule(name: &str, when: &str, fee: &str) -> String {
    format!("[[rule]]\nname = '{name}'\nwhen = '{when}'\nfee = '{fee}'\n")
}

#[test]
fn mainnet_sample_loads_in_file_order() {
    let rules = load_rules(RULES).unwrap();
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
    let rules = load_rules(RULES).unwrap();
    let evaluator = rules.evaluator(Utc::now());

    let evaluation = evaluator.evaluate(without_app_data(&order(USDC, USDT), false));
    assert!(evaluation.skipped.is_empty());
    let Match { rule, fees } = evaluation.matched.first().unwrap();
    assert_eq!(rule.name(), "Stablecoin bucket: 0.3 bps");
    assert!(matches!(
        fees[..],
        [Fee::Surplus { .. }, Fee::Volume { factor }] if factor.to_high_precision() == 30
    ));

    let mut quoted = order(USDC, USDT);
    quoted.metadata.quote = Some(Default::default());
    let evaluation = evaluator.evaluate(without_app_data(&quoted, true));
    let Match { fees, .. } = evaluation.matched.first().unwrap();
    assert!(matches!(
        fees[..],
        [Fee::PriceImprovement { .. }, Fee::Volume { .. }]
    ));

    let evaluation = evaluator.evaluate(without_app_data(&order(WETH, USDC), false));
    let Match { rule, .. } = evaluation.matched.first().unwrap();
    assert_eq!(rule.name(), "Default: 2 bps");
}

#[test]
fn empty_file_has_no_rules() {
    assert!(load_rules("").unwrap().rules().is_empty());
}

#[test]
fn refuses_bad_files() {
    let error = |source: &str| load_rules(source).unwrap_err();

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
    assert!(matches!(
        error("[lists]\norder = []"),
        LoadError::Rules(CompileError::ReservedList { .. })
    ));
    let LoadError::SampleOrder(errors) =
        error(&(rule("x", "false", "volume(0.001)") + &rule("y", "order.sellAmount", "no_fee()")))
    else {
        panic!("expected a sample order error");
    };
    let failed: Vec<_> = errors
        .iter()
        .map(|error| (error.rule.as_str(), error.field))
        .collect();
    assert_eq!(failed, [("x", Field::Fee), ("y", Field::When)]);
}
