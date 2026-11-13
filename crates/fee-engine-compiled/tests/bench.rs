//! Real-auction benchmark: the CEL engine against the compiled rules. The
//! README has the command.

use {
    alloy_primitives::{Address, U256},
    chrono::Utc,
    fee_engine::{AppData, Fee, Field, OrderView, RuleSetDefinition},
    model::order::{Order, OrderData, OrderKind, OrderMetadata},
    serde_json::Value,
    std::{
        hint::black_box,
        time::{Duration, Instant},
    },
};

struct Input {
    order: Order,
    app_data: AppData,
    in_market: bool,
}

fn read(var: &str) -> Value {
    serde_json::from_slice(&std::fs::read(std::env::var(var).unwrap()).unwrap()).unwrap()
}

fn inputs() -> Vec<Input> {
    let u256 = |v: &Value| U256::from_str_radix(v.as_str().unwrap(), 10).unwrap();
    let address = |v: &Value| -> Address { v.as_str().unwrap().parse().unwrap() };
    read("FEE_BENCH_ORDERS")
        .as_array()
        .unwrap()
        .iter()
        .map(|o| {
            let order = Order {
                data: OrderData {
                    sell_token: address(&o["sell_token"]),
                    buy_token: address(&o["buy_token"]),
                    sell_amount: u256(&o["sell_amount"]),
                    buy_amount: u256(&o["buy_amount"]),
                    valid_to: u32::try_from(o["valid_to"].as_u64().unwrap()).unwrap(),
                    kind: if o["kind"] == "sell" {
                        OrderKind::Sell
                    } else {
                        OrderKind::Buy
                    },
                    partially_fillable: o["partially_fillable"].as_bool().unwrap(),
                    ..Default::default()
                },
                metadata: OrderMetadata {
                    owner: address(&o["owner"]),
                    ..Default::default()
                },
                ..Default::default()
            };
            let app_code = o["app_data"]
                .as_str()
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .and_then(|doc| doc["appCode"].as_str().map(str::to_owned));
            // In market: the limit price is no worse than the quote's.
            let in_market = o["quote"].is_object() && {
                let quote = &o["quote"];
                order
                    .data
                    .sell_amount
                    .widening_mul::<256, 4, 512, 8>(u256(&quote["buy_amount"]))
                    >= u256(&quote["sell_amount"])
                        .widening_mul::<256, 4, 512, 8>(order.data.buy_amount)
            };
            Input {
                order,
                app_data: AppData {
                    app_code,
                    flashloan: false,
                },
                in_market,
            }
        })
        .collect()
}

/// The mainnet policy over the prod buckets, in three shapes.
fn rules(shape: &str) -> String {
    let buckets = read("FEE_BENCH_BUCKETS");
    let buckets = buckets.as_array().unwrap();
    let mut rules = String::from(
        "[lists]\njit_owners = []\nwrapped_native = \
         [\"0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2\"]\n",
    );
    for (i, tokens) in buckets.iter().enumerate() {
        rules += &format!("bucket_{i} = {tokens}\n");
    }
    let mut extra = Vec::new();
    if shape == "extended" {
        extra = vec![
            (
                "trust wallet",
                r#"order.appData.?appCode.orValue("").lowerAscii() == "trust wallet""#,
                "no_fee()",
            ),
            (
                "large sells",
                r#"order.kind == "sell" && order.sellAmount > 1e24 && !order.partiallyFillable"#,
                "volume(1)",
            ),
            (
                "long lived",
                r#"timestamp(int(order.validTo)) - now > duration("720h")"#,
                "surplus(50, 98)",
            ),
        ];
    }
    rules +=
        "\n[[rule]]\nname = \"jit\"\nwhen = \"order.owner in jit_owners\"\nfee = \"no_fee()\"\n";
    for (name, when, fee) in &extra {
        rules += &format!("\n[[rule]]\nname = \"{name}\"\nwhen = '{when}'\nfee = \"{fee}\"\n");
    }
    let split = shape != "ternary";
    let mut add = |name: &str, when: &str, volume: &str| {
        let volume = if volume.is_empty() {
            String::new()
        } else {
            format!(", {volume}")
        };
        if split {
            let when_market = if when == "true" {
                "order.inMarket".to_owned()
            } else {
                format!("({when}) && order.inMarket")
            };
            rules += &format!(
                "\n[[rule]]\nname = \"{name} (market)\"\nwhen = '{when_market}'\nfee = \
                 \"[price_improvement(50, 98){volume}]\"\n"
            );
            rules += &format!(
                "\n[[rule]]\nname = \"{name}\"\nwhen = '{when}'\nfee = \"[surplus(50, \
                 98){volume}]\"\n"
            );
        } else {
            rules += &format!(
                "\n[[rule]]\nname = \"{name}\"\nwhen = '{when}'\nfee = \"[order.inMarket ? \
                 price_improvement(50, 98) : surplus(50, 98){volume}]\"\n"
            );
        }
    };
    add(
        "same",
        "order.sellToken == order.buyToken || (order.sellToken in wrapped_native && \
         order.buyToken == \"0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\")",
        "",
    );
    for i in 0..buckets.len() {
        add(
            &format!("bucket {i}"),
            &format!("order.sellToken in bucket_{i} && order.buyToken in bucket_{i}"),
            "volume(0.3)",
        );
    }
    add("default", "true", "volume(2)");
    rules
}

type Outcome = (Vec<(String, Vec<Fee>)>, Vec<(String, Field)>);

/// Mean and best of 30 passes over every order, in µs per order.
fn time(orders: usize, mut f: impl FnMut()) -> (f64, f64) {
    const PASSES: u32 = 30;
    f();
    let (mut total, mut best) = (Duration::ZERO, Duration::MAX);
    for _ in 0..PASSES {
        let start = Instant::now();
        f();
        let elapsed = start.elapsed();
        total += elapsed;
        best = best.min(elapsed);
    }
    #[expect(clippy::cast_precision_loss, reason = "order counts are small")]
    let per_order = |d: Duration| d.as_secs_f64() * 1e6 / orders as f64;
    (per_order(total / PASSES), per_order(best))
}

#[test]
#[ignore]
fn bench_real_auction() {
    let inputs = inputs();
    let views: Vec<_> = inputs
        .iter()
        .map(|input| OrderView {
            order: &input.order,
            app_data: &input.app_data,
            partner_fees: &[],
            in_market: input.in_market,
        })
        .collect();
    let n = views.len();
    let in_market = inputs.iter().filter(|i| i.in_market).count();
    let app_codes = inputs
        .iter()
        .filter(|i| i.app_data.app_code.is_some())
        .count();
    println!("orders: {n} (in market {in_market}, with appCode {app_codes})\n");
    println!(
        "{:<10} {:>5}  {:>20}  {:>20}  {:>8}",
        "rules", "count", "cel mean/best us", "compiled mean/best", "speedup"
    );
    for shape in ["ternary", "split", "extended"] {
        let definition: RuleSetDefinition = toml::from_str(&rules(shape)).unwrap();
        let count = definition.rules.len();
        let cel = fee_engine::RuleSet::new(definition.clone()).unwrap();
        let compiled = fee_engine_compiled::RuleSet::new(definition).unwrap();
        let now = Utc::now();
        let (cel, compiled) = (cel.evaluator(now), compiled.evaluator(now));

        let mut matches = 0;
        for view in &views {
            let expected = cel.evaluate(*view);
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
            let actual = compiled.evaluate(*view);
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
            assert_eq!(actual, expected, "{shape}: {:?}", view.order);
            matches += actual.0.len();
        }

        let (cel_mean, cel_best) = time(n, || {
            for view in &views {
                black_box(cel.evaluate(*view));
            }
        });
        let (mean, best) = time(n, || {
            for view in &views {
                black_box(compiled.evaluate(*view));
            }
        });
        println!(
            "{shape:<10} {count:>5}  {cel_mean:>9.3} / {cel_best:>7.3}  {mean:>9.3} / {best:>7.3}  {:>7.1}x   ({matches} matches, 0 mismatches)",
            cel_mean / mean
        );
    }
}
