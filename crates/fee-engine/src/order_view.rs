//! The order as a rule sees it.

use {
    model::order::Order,
    serde_json::{Number, Value},
};

/// The app-data document as text; never a number even when it looks like one.
const FULL_APP_DATA: &str = "fullAppData";

/// The order as rules see it: the API JSON with decimal strings as doubles,
/// hex strings in lowercase and `fastPath` always present. The model
/// serializes every amount and price as a decimal string, so no field list
/// is kept here.
pub(crate) fn order_view(order: &Order) -> Value {
    let mut view = serde_json::to_value(order).expect("orders serialize");
    // The model omits `fastPath` when false; a rule reading it must not fail
    // on every ordinary order.
    view["fastPath"] = order.metadata.fast_path.into();
    if let Some(quote) = &order.metadata.quote {
        // `BigDecimal` displays small values as `1E-7`; the plain form is
        // what `convert` reads.
        view["quote"]["gasAmount"] = quote.gas_amount.to_plain_string().into();
        view["quote"]["gasPrice"] = quote.gas_price.to_plain_string().into();
        view["quote"]["sellTokenPrice"] = quote.sell_token_price.to_plain_string().into();
    }
    convert(&mut view);
    view
}

/// Walks the JSON in place: decimal strings become numbers, hex strings are
/// lowercased, and `fullAppData` is left as written.
fn convert(value: &mut Value) {
    match value {
        Value::String(text) if is_decimal(text) => {
            if let Some(number) = text.parse::<f64>().ok().and_then(Number::from_f64) {
                *value = Value::Number(number);
            }
        }
        Value::String(text) if is_hex(text) => text.make_ascii_lowercase(),
        Value::Array(items) => items.iter_mut().for_each(convert),
        Value::Object(fields) => fields
            .iter_mut()
            .filter(|(key, _)| key.as_str() != FULL_APP_DATA)
            .for_each(|(_, value)| convert(value)),
        _ => {}
    }
}

/// Digits with at most one `.` inside, as `U256`, `BigUint` and a plain
/// `BigDecimal` display. Narrower than `f64::from_str`, which also takes
/// `inf`, `1e5` and `.5`.
fn is_decimal(text: &str) -> bool {
    let (int, frac) = text.split_once('.').unwrap_or((text, "0"));
    let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
    digits(int) && digits(frac)
}

fn is_hex(text: &str) -> bool {
    text.strip_prefix("0x").is_some_and(const_hex::check_raw)
}

/// Lowercases hex strings so they compare equal to the lowercased order view.
pub(crate) fn normalize(text: &str) -> String {
    if is_hex(text) {
        text.to_ascii_lowercase()
    } else {
        text.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy_primitives::{U256, address},
        model::order::{OrderData, OrderMetadata, OrderQuote},
    };

    #[test]
    fn order_view_uses_api_names_numbers_and_lowercase_hex() {
        let order = Order {
            metadata: OrderMetadata {
                owner: address!("0xAbCdEf0000000000000000000000000000000001"),
                ..Default::default()
            },
            data: OrderData {
                sell_amount: "1000000000000000000000".parse().unwrap(),
                ..Default::default()
            },
            ..Default::default()
        };
        let view = order_view(&order);

        assert_eq!(view["owner"], "0xabcdef0000000000000000000000000000000001");
        assert_eq!(view["sellAmount"], 1e21);
        assert_eq!(view["kind"], "buy");
        assert_eq!(view["class"], "limit");
        assert_eq!(view["signingScheme"], "eip712");
        assert_eq!(view["appData"], format!("0x{}", "0".repeat(64)));
        assert_eq!(view["fastPath"], false);
    }

    /// Every field the model emits as a decimal string is an amount or a
    /// price; a digit-only string that is not one must stay a string.
    #[test]
    fn every_decimal_string_is_a_number_and_nothing_else_is() {
        let order = Order {
            metadata: OrderMetadata {
                quote: Some(OrderQuote {
                    gas_price: "1234.5".parse().unwrap(),
                    sell_token_price: "0.0000001".parse().unwrap(),
                    ..Default::default()
                }),
                available_balance: Some(U256::from(7)),
                gas_cost: Some(U256::from(8)),
                executed_sell_amount: 9u64.into(),
                ethflow_data: Some(Default::default()),
                onchain_order_data: Some(Default::default()),
                full_app_data: Some("123".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        let view = order_view(&order);

        assert_eq!(view["quote"]["gasPrice"], 1234.5);
        assert_eq!(view["quote"]["sellTokenPrice"], 1e-7);
        assert_eq!(view["quote"]["sellAmount"], 0.0);
        assert_eq!(view["availableBalance"], 7.0);
        assert_eq!(view["gasCost"], 8.0);
        assert_eq!(view["executedSellAmount"], 9.0);
        assert_eq!(view["fullAppData"], "123");

        let mut decimal_strings = vec![];
        collect_decimal_strings("order", &view, &mut decimal_strings);
        assert_eq!(decimal_strings, Vec::<String>::new());
    }

    fn collect_decimal_strings(path: &str, value: &Value, found: &mut Vec<String>) {
        match value {
            Value::String(text) if is_decimal(text) => found.push(path.to_owned()),
            Value::Array(items) => items.iter().enumerate().for_each(|(i, item)| {
                collect_decimal_strings(&format!("{path}[{i}]"), item, found)
            }),
            Value::Object(fields) => fields
                .iter()
                .filter(|(key, _)| key.as_str() != FULL_APP_DATA)
                .for_each(|(key, value)| {
                    collect_decimal_strings(&format!("{path}.{key}"), value, found)
                }),
            _ => {}
        }
    }

    #[test]
    fn decimal_is_narrower_than_f64() {
        for text in ["0", "42", "1234.5", "0.5", "0.0000001"] {
            assert!(is_decimal(text), "{text}");
        }
        for text in [
            "", ".", "1.", ".5", "1e5", "1E-7", "inf", "nan", "-1", "0x1", "1.2.3", "1_000",
        ] {
            assert!(!is_decimal(text), "{text}");
        }
    }

    #[test]
    fn normalizes_only_hex() {
        assert_eq!(normalize("0xAB"), "0xab");
        assert_eq!(normalize("Acme"), "Acme");
        assert_eq!(normalize("0x"), "0x");
        assert_eq!(normalize("0xzz"), "0xzz");
    }
}
