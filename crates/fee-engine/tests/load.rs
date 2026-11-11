//! The rules file loader through its public API only.

use {
    alloy_primitives::{Address, U256, address},
    chrono::Utc,
    fee_engine::{
        AppData,
        CompileError,
        Fee,
        FeeFactor,
        Field,
        LoadError,
        Match,
        OrderView,
        PartnerFee,
        RuleSet,
        RuleSetDefinition,
        load_rules,
        load_rules_from_str,
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

    let evaluation = evaluator.evaluate(OrderView {
        order: &order(USDC, USDT),
        app_data: &AppData::default(),
        partner_fees: &[],
        in_market: false,
    });
    assert!(evaluation.skipped.is_empty());
    let Match { rule, fees } = evaluation.matched.first().unwrap();
    assert_eq!(rule.name(), "Stablecoin bucket: 0.3 bps");
    assert!(matches!(
        fees[..],
        [Fee::Surplus { .. }, Fee::Volume { factor }] if factor == FeeFactor::try_from(0.00003).unwrap()
    ));

    let mut quoted = order(USDC, USDT);
    quoted.metadata.quote = Some(Default::default());
    let evaluation = evaluator.evaluate(OrderView {
        order: &quoted,
        app_data: &AppData::default(),
        partner_fees: &[],
        in_market: true,
    });
    let Match { fees, .. } = evaluation.matched.first().unwrap();
    assert!(matches!(
        fees[..],
        [Fee::PriceImprovement { .. }, Fee::Volume { .. }]
    ));

    let evaluation = evaluator.evaluate(OrderView {
        order: &order(WETH, USDC),
        app_data: &AppData::default(),
        partner_fees: &[],
        in_market: false,
    });
    let Match { rule, .. } = evaluation.matched.first().unwrap();
    assert_eq!(rule.name(), "Default: 2 bps");
}

/// The partner fees of an order asking for one 2% volume fee, under `source`.
fn partner_fee_under(source: &str) -> Vec<Fee> {
    load_rules_from_str(source)
        .unwrap()
        .evaluator(Utc::now())
        .evaluate(OrderView {
            order: &order(WETH, USDC),
            app_data: &AppData::default(),
            partner_fees: &[PartnerFee::Volume { bps: 200 }],
            in_market: false,
        })
        .partner_fees
}

#[test]
fn partner_fee_cap_is_in_bps_and_100_by_default() {
    let volume = |bps: f64| Fee::Volume {
        factor: (bps / 10_000.).try_into().unwrap(),
    };
    assert_eq!(partner_fee_under(""), [volume(100.)]);
    assert_eq!(
        partner_fee_under("[partner-fees]\nmax-volume-bps = 50"),
        [volume(50.)]
    );
}

#[test]
fn empty_file_has_no_rules() {
    assert!(load_rules_from_str("").unwrap().rules().is_empty());
}

#[test]
fn a_definition_loads_like_the_file() {
    let names = |rules: RuleSet| -> Vec<String> {
        rules
            .rules()
            .iter()
            .map(|rule| rule.name().to_owned())
            .collect()
    };
    let section: RuleSetDefinition = toml::from_str(RULES).unwrap();
    let rules = load_rules(section).unwrap();
    assert_eq!(rules.max_partner_fee(), FeeFactor::try_from(0.05).unwrap());
    assert_eq!(names(rules), names(load_rules_from_str(RULES).unwrap()));

    let empty = load_rules(RuleSetDefinition::default()).unwrap();
    assert!(empty.rules().is_empty());
    assert_eq!(empty.max_partner_fee(), FeeFactor::try_from(0.01).unwrap());
}

#[test]
fn a_section_round_trips_through_toml() {
    let section: RuleSetDefinition = toml::from_str(RULES).unwrap();
    let written = toml::to_string(&section).unwrap();
    assert_eq!(
        toml::from_str::<RuleSetDefinition>(&written).unwrap(),
        section
    );
}

#[test]
fn refuses_bad_files() {
    let error = |source: &str| load_rules_from_str(source).unwrap_err();

    assert!(matches!(error("[lists"), LoadError::Toml(_)));
    assert!(matches!(error("[caps]\nvolume = 1"), LoadError::Toml(_)));
    for partner_fees in [
        "[partner-fees]",
        "[partner-fees]\nmax = 1",
        "[partner-fees]\nmax-volume-bps = 10000",
        "[partner-fees]\nmax-volume-bps = -1",
        "[partner-fees]\nmax-volume-bps = 0.001",
    ] {
        assert!(
            matches!(error(partner_fees), LoadError::Toml(_)),
            "{partner_fees}"
        );
    }
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
