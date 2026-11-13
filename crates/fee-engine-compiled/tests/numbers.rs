//! Exact numbers: where the compiled rules and the CEL engine's doubles
//! differ, and the limits of the decimal type.

use {
    alloy_primitives::U256,
    chrono::Utc,
    fee_engine::{AppData, Fee, FeeFactor, OrderView, RuleDefinition, RuleSetDefinition},
    model::order::{Order, OrderData},
    rust_decimal::Decimal,
};

fn order(sell: U256, buy: U256) -> Order {
    Order {
        data: OrderData {
            sell_amount: sell,
            buy_amount: buy,
            valid_to: 1_800_000_000,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// `Ok(fees)` when the rule matched, `Ok(None)` when not, `Err` when skipped.
fn evaluate(when: &str, fee: &str, order: &Order) -> Result<Option<Vec<Fee>>, String> {
    let definition = RuleSetDefinition {
        rules: vec![RuleDefinition {
            name: "r".into(),
            when: when.into(),
            fee: fee.into(),
        }],
        ..Default::default()
    };
    let rules = fee_engine_compiled::RuleSet::new(definition).map_err(|err| err.to_string())?;
    let evaluation = rules.evaluator(Utc::now()).evaluate(OrderView {
        order,
        app_data: &AppData::default(),
        partner_fees: &[],
        in_market: false,
    });
    if let Some(skipped) = evaluation.skipped.first() {
        return Err(skipped.error.to_string());
    }
    Ok(evaluation.matched.into_iter().next().map(|m| m.fees))
}

fn matches(when: &str, order: &Order) -> bool {
    evaluate(when, "no_fee()", order).unwrap().is_some()
}

fn e(exponent: u64) -> U256 {
    U256::from(10).pow(U256::from(exponent))
}

fn volume(bps: &str) -> Vec<Fee> {
    let bps: Decimal = bps.parse().unwrap();
    vec![Fee::Volume {
        factor: FeeFactor::try_from(bps / Decimal::from(10_000)).unwrap(),
    }]
}

#[test]
fn amounts_compare_exactly() {
    // As a double, 10^21 + 1 is 1e21.
    let order = order(e(21) + U256::from(1), U256::from(1));
    assert!(matches("order.sellAmount > 1e21", &order));
    assert!(!matches("order.sellAmount == 1e21", &order));
    assert!(matches(
        "order.sellAmount - order.buyAmount == 1e21",
        &order
    ));
    // 10^30 does not fit a decimal; comparing with a fraction still works.
    let order = self::order(e(30), U256::from(1));
    assert!(matches("order.sellAmount > 2.5", &order));
    assert!(matches("order.sellAmount > -1", &order));
    assert!(matches("order.buyAmount < 1.5", &order));
}

#[test]
fn decimal_literals_are_read_as_written() {
    // A real mainnet order. As doubles, the amount and 1e24 are equal.
    let order = order("999999999999999984204664".parse().unwrap(), U256::from(1));
    assert!(matches("order.sellAmount < 1e24", &order));
    assert!(!matches("order.sellAmount > 1e24", &order));
    assert!(matches(
        "order.sellAmount > 9.99999999999999984204663e23",
        &order
    ));
    // Too large for a decimal: kept as an integer.
    let order = self::order(e(30), U256::from(1));
    assert!(matches("order.sellAmount == 1e30", &order));
    assert!(matches("order.sellAmount > -1e30", &order));
    assert!(matches("1.0e-2 == 0.01", &order));
}

#[test]
fn decimal_arithmetic_is_exact() {
    let order = order(U256::from(3) * e(19), U256::from(1));
    // As doubles, 0.1 + 0.2 is 0.30000000000000004: off the fee grid.
    assert_eq!(
        evaluate("true", "volume(0.1 + 0.2)", &order),
        Ok(Some(volume("0.3")))
    );
    assert_eq!(
        evaluate("true", "volume(order.sellAmount / 1e20)", &order),
        Ok(Some(volume("0.3")))
    );
    assert_eq!(
        evaluate("true", "volume(math.round(2.5))", &order),
        Ok(Some(volume("3")))
    );
}

#[test]
fn integer_arithmetic_keeps_its_type() {
    let order = order(U256::from(7), U256::from(2));
    // A uint stays a uint with an int constant, and divides as integers.
    assert!(matches("order.sellAmount / order.buyAmount == 3", &order));
    assert!(matches("order.validTo - 3600 > 0u", &order));
    // With a decimal constant, it divides as decimals.
    assert!(matches("order.sellAmount / 2.0 == 3.5", &order));
    assert!(matches("int(order.buyAmount) - 5 == -3", &order));
}

#[test]
fn out_of_range_arithmetic_skips_the_rule() {
    let order = order(e(30), U256::from(1));
    // A uint does not go below zero.
    assert_eq!(
        evaluate(
            "order.buyAmount - order.sellAmount > 0u",
            "no_fee()",
            &order
        ),
        Err("arithmetic overflowed or divided by zero".into())
    );
    assert_eq!(
        evaluate("order.sellAmount / 0u > 0u", "no_fee()", &order),
        Err("arithmetic overflowed or divided by zero".into())
    );
    // Decimal arithmetic needs the amount to fit 96 bits.
    assert_eq!(
        evaluate("order.sellAmount / 1e20 > 1.0", "no_fee()", &order),
        Err("the number does not fit a decimal".into())
    );
    // A constant that does not fit its type is refused when the rules load.
    assert!(
        evaluate("order.sellAmount > 0u", "volume(1.0 / 0.0)", &order)
            .unwrap_err()
            .contains("does not compile")
    );
}
