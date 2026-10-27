//! The engine through its public API only.

use {
    alloy_primitives::{Address, U256, address},
    chrono::{TimeZone, Utc},
    configs::fee_factor::FeeFactor,
    fee_engine::{Caps, Fee, Match, RuleDefinition, RuleSet},
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

fn rules() -> RuleSet {
    let definitions = vec![
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
    ];
    let lists = BTreeMap::from([
        (
            "stablecoins".to_owned(),
            vec![USDC.to_string(), USDT.to_string()],
        ),
        ("jit_makers".to_owned(), vec![MAKER.to_string()]),
    ]);
    let caps = Caps {
        volume_factor: FeeFactor::new(0.01),
        share_factor: FeeFactor::new(0.9),
        max_volume_factor: FeeFactor::new(0.05),
    };
    RuleSet::new(definitions, lists, caps).unwrap()
}

fn order(owner: Address, sell: Address, buy: Address) -> Order {
    Order {
        metadata: OrderMetadata {
            owner,
            ..Default::default()
        },
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

fn matched(evaluation: Option<Match<'_>>) -> (String, Vec<Fee>) {
    let Match { rule, fees } = evaluation.expect("expected a match");
    (rule.name().to_owned(), fees)
}

#[test]
fn picks_the_first_matching_rule_per_order() {
    let rules = rules();
    let user = address!("0x2222222222222222222222222222222222222222");

    let before_launch = rules.evaluator(Utc.with_ymd_and_hms(2026, 11, 1, 0, 0, 0).unwrap());
    let (rule, fees) = matched(before_launch.evaluate(&order(user, USDC, USDT)));
    assert_eq!(rule, "Stablecoin launch month");
    assert_eq!(
        fees,
        vec![Fee::Volume {
            factor: FeeFactor::new(0.0001)
        }]
    );

    let after_launch = rules.evaluator(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());
    let (rule, fees) = matched(after_launch.evaluate(&order(user, USDC, USDT)));
    assert_eq!(rule, "Stablecoin swaps");
    assert_eq!(
        fees,
        vec![Fee::Volume {
            factor: FeeFactor::new(0.001)
        }]
    );

    let (rule, fees) = matched(after_launch.evaluate(&order(user, WETH, USDT)));
    assert_eq!(rule, "Default");
    assert_eq!(
        fees,
        vec![Fee::Surplus {
            factor: FeeFactor::new(0.5),
            max_volume_factor: FeeFactor::new(0.01),
        }]
    );

    let (rule, fees) = matched(after_launch.evaluate(&order(MAKER, USDC, USDT)));
    assert_eq!(rule, "JIT makers");
    assert_eq!(fees, vec![]);
}

#[test]
fn no_matching_rule_means_no_fee() {
    let rules = RuleSet::new(
        vec![rule("Sells only", r#"order.kind == "sell""#, "volume(1)")],
        BTreeMap::new(),
        Caps {
            volume_factor: FeeFactor::new(0.01),
            share_factor: FeeFactor::new(0.9),
            max_volume_factor: FeeFactor::new(0.05),
        },
    )
    .unwrap();
    let evaluator = rules.evaluator(Utc::now());
    assert!(evaluator.evaluate(&Order::default()).is_none());
}
