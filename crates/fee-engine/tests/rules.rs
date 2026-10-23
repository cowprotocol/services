use {
    alloy_primitives::{U256, address},
    chrono::{TimeZone, Utc},
    configs::fee_factor::FeeFactor,
    fee_engine::{Caps, Evaluation, Fact, Facts, Fee, RuleSet, Settings},
    model::{
        order::{Order, OrderData, OrderMetadata},
        signature::Signature,
    },
};

const RULES: &str = include_str!("rules.toml");

fn settings() -> Settings {
    Settings {
        caps: Caps::default(),
    }
}

fn rules() -> RuleSet {
    RuleSet::load(RULES, settings()).unwrap()
}

fn order() -> Order {
    Order {
        metadata: OrderMetadata {
            owner: address!("0x9999999999999999999999999999999999999999"),
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
    assert_eq!(rules.rules().len(), 8);
    assert_eq!(rules.rules()[0].name(), "Exempt JIT makers");
    assert_eq!(rules.rules()[0].fee(), "no_fee()");
}

#[test]
fn matches_without_market_data_when_no_rule_needs_it() {
    let rules = rules();
    let auction = rules.auction(Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap());

    let Evaluation::Matched { rule, fees } = auction.evaluate(&order(), &Facts::default()) else {
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

    // A smart-wallet buy order does not match the sell rule, so the `size_usd`
    // its fee reads is never needed.
    let mut smart_wallet_buy = order();
    smart_wallet_buy.signature = Signature::Eip1271(vec![]);
    assert!(matches!(
        auction.evaluate(&smart_wallet_buy, &Facts::default()),
        Evaluation::Matched { rule, .. } if rule.name() == "Correlated pairs"
    ));

    // A rule that reads `size_usd` suspends until the fact is supplied.
    let rules = RuleSet::load(
        "[[rule]]\nname = 'Big'\nwhen = 'size_usd > 1e6'\nfee = 'volume(1)'",
        settings(),
    )
    .unwrap();
    let auction = rules.auction(Utc::now());
    assert!(matches!(
        auction.evaluate(&order(), &Facts::default()),
        Evaluation::Suspended {
            missing: Fact::SizeUsd,
            ..
        }
    ));
    assert!(matches!(
        auction.evaluate(
            &order(),
            &Facts {
                size_usd: Some(2e6)
            }
        ),
        Evaluation::Matched { .. }
    ));
    assert!(matches!(
        auction.evaluate(
            &order(),
            &Facts {
                size_usd: Some(2e3)
            }
        ),
        Evaluation::NoMatch
    ));
}
