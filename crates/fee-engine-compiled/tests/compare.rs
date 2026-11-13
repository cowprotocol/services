//! Runs `fee_engine` (the CEL interpreter) and this crate on the same rules
//! and orders and asserts they agree on every match, fee and skip.

use {
    alloy_primitives::{Address, U256, address},
    chrono::{DateTime, TimeDelta, TimeZone, Utc},
    fee_engine::{AppData, Fee, Field, OrderView, RuleSetDefinition},
    model::{
        order::{EthflowData, Order, OrderData, OrderKind, OrderMetadata},
        signature::Signature,
    },
};

const USDC: Address = address!("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48");
const USDT: Address = address!("0xdAC17F958D2ee523a2206206994597C13D831ec7");
const WETH: Address = address!("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2");
const ETH: Address = address!("0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE");
const MAKER: Address = address!("0x1111111111111111111111111111111111111111");

const RULES: &str = r#"
[lists]
stablecoins = ["0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", "0xdac17f958d2ee523a2206206994597c13d831ec7"]
jit = ["0x1111111111111111111111111111111111111111"]
wrapped_native = ["0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"]
codes = ["CoW Swap", "Trust Wallet"]

[[rule]]
name = "jit"
when = "order.owner in jit"
fee = "no_fee()"

[[rule]]
name = "same"
when = 'order.sellToken == order.buyToken || (order.sellToken in wrapped_native && order.buyToken == "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee")'
fee = "order.inMarket ? price_improvement(50, 98) : surplus(50, 98)"

[[rule]]
name = "stable"
when = "order.sellToken in stablecoins && order.buyToken in stablecoins"
fee = "[order.inMarket ? price_improvement(50, 98) : surplus(50, 98), volume(0.3)]"

[[rule]]
name = "trust"
when = 'order.appData.?appCode.orValue("").lowerAscii() == "trust wallet"'
fee = "no_fee()"

[[rule]]
name = "code in list"
when = "order.appData.appCode in codes"
fee = "volume(0.5)"

[[rule]]
name = "large sell"
when = 'order.kind == "sell" && order.sellAmount > 1e21'
fee = "surplus(order.partiallyFillable ? 40 : 50, 100)"

[[rule]]
name = "eip1271"
when = 'order.signingScheme == "eip1271"'
fee = 'volume(order.kind == "sell" ? 2 : 5)'

[[rule]]
name = "ethflow plain"
when = "order.ethflowData.userValidTo > 0"
fee = "volume(1)"

[[rule]]
name = "ethflow optional"
when = "order.?ethflowData.hasValue() || order.?ethflowData.userValidTo.orValue(0u) > 5u"
fee = "volume(1.5)"

[[rule]]
name = "flashloan"
when = "has(order.appData.metadata.flashloan)"
fee = "volume(3)"

[[rule]]
name = "valid from"
when = "has(order.validFrom) && order.validFrom < order.validTo"
fee = "volume(4)"

[[rule]]
name = "launch"
when = 'now < timestamp("2026-11-14T00:00:00Z")'
fee = "volume(1)"

[[rule]]
name = "expiry"
when = 'timestamp(int(order.validTo)) - now > duration("1h")'
fee = "volume(6)"

[[rule]]
name = "old"
when = 'now > timestamp(order.creationDate) + duration("24h")'
fee = "volume(10)"

[[rule]]
name = "computed"
when = "order.buyAmount > 0.0"
fee = "volume(math.greatest(1.0, math.least(10.0, order.sellAmount / 1e20)))"

[[rule]]
name = "rounded"
when = "order.buyAmount > 0.0"
fee = "volume(math.floor(math.least(10.0, order.sellAmount / 1e20)))"

[[rule]]
name = "owner text"
when = 'order.owner.startsWith("0x11") || order.receiver.endsWith("ab")'
fee = "volume(7)"

[[rule]]
name = "receiver"
when = 'order.receiver == "0x0000000000000000000000000000000000000000"'
fee = "volume(8)"

[[rule]]
name = "fast"
when = "order.fastPath || !order.partiallyFillable"
fee = "volume(9)"

[[rule]]
name = "default"
when = "true"
fee = "[surplus(50, 100), volume(2)]"
"#;

/// A small deterministic generator, so a failure reproduces.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> T {
        items[usize::try_from(self.next()).unwrap() % items.len()]
    }
}

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 13, 0, 0, 0).unwrap()
}

fn orders(count: usize) -> Vec<(Order, AppData, bool)> {
    let mut rng = Rng(42);
    let tokens = [
        USDC,
        USDT,
        WETH,
        ETH,
        address!("0x00000000000000000000000000000000000000ab"),
    ];
    let owners = [
        MAKER,
        address!("0x2222222222222222222222222222222222222222"),
    ];
    let codes = [
        None,
        Some("CoW Swap"),
        Some("Trust Wallet"),
        Some("trust wallet"),
        Some("x"),
    ];
    (0..count)
        .map(|_| {
            let amount = |rng: &mut Rng| {
                U256::from(rng.next()) * U256::from(10).pow(U256::from(rng.next() % 12 + 8))
            };
            let order = Order {
                data: OrderData {
                    sell_token: rng.pick(&tokens),
                    buy_token: rng.pick(&tokens),
                    sell_amount: amount(&mut rng),
                    buy_amount: amount(&mut rng),
                    receiver: rng
                        .pick(&[
                            Address::ZERO,
                            address!("0x00000000000000000000000000000000000000ab"),
                        ])
                        .into(),
                    valid_to: u32::try_from(now().timestamp()).unwrap() + rng.pick(&[0, 60, 7200]),
                    kind: rng.pick(&[OrderKind::Buy, OrderKind::Sell]),
                    partially_fillable: rng.pick(&[true, false]),
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    owner: rng.pick(&owners),
                    creation_date: now() - TimeDelta::hours(rng.pick(&[1, 30])),
                    valid_from: rng.pick(&[None, Some(1), Some(u32::MAX)]),
                    fast_path: rng.pick(&[true, false]),
                    ethflow_data: rng.pick(&[None, Some(0), Some(10)]).map(|user_valid_to| {
                        EthflowData {
                            user_valid_to,
                            refund_tx_hash: None,
                        }
                    }),
                    ..Default::default()
                },
                signature: if rng.next().is_multiple_of(2) {
                    Signature::Eip1271(vec![])
                } else {
                    Signature::PreSign
                },
                ..Default::default()
            };
            let app_data = AppData {
                app_code: rng.pick(&codes).map(str::to_owned),
                flashloan: rng.pick(&[true, false]),
            };
            (order, app_data, rng.pick(&[true, false]))
        })
        .collect()
}

type Outcome = (Vec<(String, Vec<Fee>)>, Vec<(String, Field)>);

#[test]
fn agrees_with_the_cel_engine() {
    let definition: RuleSetDefinition = toml::from_str(RULES).unwrap();
    let cel = fee_engine::RuleSet::new(definition.clone()).unwrap();
    let compiled = fee_engine_compiled::RuleSet::new(definition).unwrap();
    // Rule name -> orders it matched or skipped on.
    let mut matched = std::collections::BTreeMap::<String, usize>::new();
    for at in [now(), now() + TimeDelta::days(2)] {
        let (cel, compiled) = (cel.evaluator(at), compiled.evaluator(at));
        let mut skipped = 0;
        for (index, (order, app_data, in_market)) in orders(5_000).iter().enumerate() {
            let view = OrderView {
                order,
                app_data,
                partner_fees: &[],
                in_market: *in_market,
            };
            let expected = cel.evaluate(view);
            let expected: Outcome = (
                expected
                    .matched
                    .iter()
                    .map(|m| (m.rule.name().to_owned(), m.fees.clone()))
                    .collect(),
                expected
                    .skipped
                    .iter()
                    .map(|s| (s.rule.name().to_owned(), s.field))
                    .collect(),
            );
            let actual = compiled.evaluate(view);
            let actual: Outcome = (
                actual
                    .matched
                    .iter()
                    .map(|m| (m.rule.name().to_owned(), m.fees.clone()))
                    .collect(),
                actual
                    .skipped
                    .iter()
                    .map(|s| (s.rule.name().to_owned(), s.field))
                    .collect(),
            );
            skipped += actual.1.len();
            let names = actual
                .0
                .iter()
                .map(|(name, _)| name)
                .chain(actual.1.iter().map(|(name, _)| name));
            for name in names {
                *matched.entry(name.clone()).or_default() += 1;
            }
            assert_eq!(
                actual, expected,
                "order {index} at {at}: {order:?} {app_data:?}"
            );
        }
        assert!(skipped > 0, "the orders exercise the skip path");
    }
    assert_eq!(
        matched.len(),
        definition_rule_count(),
        "every rule matches or skips somewhere"
    );
}

fn definition_rule_count() -> usize {
    RULES.matches("[[rule]]").count()
}

#[test]
fn refuses_what_it_does_not_support() {
    let refused = |when: &str| {
        let definition = RuleSetDefinition {
            rules: vec![fee_engine::RuleDefinition {
                name: "r".into(),
                when: when.into(),
                fee: "no_fee()".into(),
            }],
            ..Default::default()
        };
        fee_engine_compiled::RuleSet::new(definition)
            .err()
            .unwrap()
            .message
    };
    assert!(refused("order.nope").contains("no field `nope`"));
    assert!(refused("unknown_list").contains("unknown name"));
    assert!(refused("[1].exists(x, x == 1)").contains("macros"));
    assert!(refused("order.sellAmount == \"a\"").contains("cannot compare"));
    assert!(refused("order.owner == \"0x12\"").contains("not one"));
    assert!(refused("1").contains("expected a bool"));
}
