use {
    alloy_primitives::{U256, address},
    chrono::{TimeZone, Utc},
    configs::fee_factor::FeeFactor,
    fee_engine::{Caps, Evaluation, Fact, Facts, Fee, RuleSet, Settings},
    model::order::{Order, OrderData, OrderMetadata},
};

const RULES: &str = include_str!("rules.toml");

fn rules() -> RuleSet {
    RuleSet::load(
        RULES,
        Settings {
            caps: Caps::default(),
            chain_id: 1,
        },
    )
    .unwrap()
}

fn order(app_data: Option<&str>) -> Order {
    Order {
        metadata: OrderMetadata {
            owner: address!("0x9999999999999999999999999999999999999999"),
            full_app_data: app_data.map(str::to_owned),
            ..Default::default()
        },
        data: OrderData {
            sell_token: address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
            buy_token: address!("0xae7ab96520DE3A18E5e111B5EaAb095312D7fE84"),
            sell_amount: U256::from(10u64).pow(U256::from(18u64)),
            buy_amount: U256::from(10u64).pow(U256::from(18u64)),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn sample_file_loads_cleanly() {
    let rules = rules();
    assert_eq!(rules.rules().len(), 13);
    assert_eq!(rules.rules()[0].name(), "Exempt JIT makers");
    assert_eq!(rules.rules()[0].fee(), "no_fee()");
}

#[test]
fn matches_without_market_data_when_no_rule_needs_it() {
    let rules = rules();
    let auction = rules.auction(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());

    let Evaluation::Matched { rule, fees } = auction.evaluate(&order(None), &Facts::default())
    else {
        panic!("expected a match");
    };
    assert_eq!(rule.name(), "Correlated pairs");
    assert_eq!(
        fees,
        vec![Fee::Volume {
            factor: FeeFactor::try_from(0.0001).unwrap()
        }]
    );
}

#[test]
fn suspends_only_when_a_reached_rule_needs_market_data() {
    let rules = rules();
    let auction = rules.auction(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());

    // Self-declared partners never reach the size-based rule.
    let claimed = order(Some(r#"{"appCode":"acme"}"#));
    assert!(matches!(
        auction.evaluate(&claimed, &Facts::default()),
        Evaluation::Matched { rule, .. } if rule.name() == "Correlated pairs"
    ));

    // A rule that reads `size_usd` suspends until the fact is supplied.
    let rules = RuleSet::load(
        "[[rule]]\nname = 'Big'\nwhen = 'size_usd > 1e6'\nfee = 'volume(1)'",
        Settings {
            caps: Caps::default(),
            chain_id: 1,
        },
    )
    .unwrap();
    let auction = rules.auction(Utc::now());
    assert!(matches!(
        auction.evaluate(&order(None), &Facts::default()),
        Evaluation::Suspended {
            missing: Fact::SizeUsd,
            ..
        }
    ));
    assert!(matches!(
        auction.evaluate(
            &order(None),
            &Facts {
                size_usd: Some(2e6)
            }
        ),
        Evaluation::Matched { .. }
    ));
    assert!(matches!(
        auction.evaluate(
            &order(None),
            &Facts {
                size_usd: Some(2e3)
            }
        ),
        Evaluation::NoMatch
    ));
}
