//! The order as a rule sees it.

use {
    alloy_primitives::hex,
    model::order::Order,
    serde_json::{Map, Value},
};

/// API order fields holding amounts as decimal strings.
const AMOUNT_FIELDS: &[&str] = &[
    "sellAmount",
    "buyAmount",
    "feeAmount",
    "executedSellAmount",
    "executedSellAmountBeforeFees",
    "executedBuyAmount",
    "executedFeeAmount",
    "executedFee",
    "availableBalance",
    "gasCost",
];

/// Fields of the order's quote holding amounts as decimal strings.
const QUOTE_AMOUNT_FIELDS: &[&str] = &[
    "sellAmount",
    "buyAmount",
    "feeAmount",
    "gasAmount",
    "gasPrice",
    "sellTokenPrice",
];

/// The order as rules see it: the API JSON with the app-data document in
/// place of its hash, amounts as doubles and hex strings in lowercase.
pub(crate) fn order_view(order: &Order, app_data: Value) -> Value {
    let mut view = serde_json::to_value(order).expect("orders serialize");
    view["appData"] = app_data;
    amounts_to_numbers(&mut view);
    lowercase_hex(&mut view);
    view
}

fn amounts_to_numbers(view: &mut Value) {
    let Some(order) = view.as_object_mut() else {
        return;
    };
    convert_fields(order, AMOUNT_FIELDS);
    if let Some(quote) = order.get_mut("quote").and_then(Value::as_object_mut) {
        convert_fields(quote, QUOTE_AMOUNT_FIELDS);
    }
}

fn convert_fields(object: &mut Map<String, Value>, fields: &[&str]) {
    for field in fields {
        let Some(value) = object.get_mut(*field) else {
            continue;
        };
        let Some(number) = value
            .as_str()
            .and_then(|text| text.parse::<f64>().ok())
            .and_then(serde_json::Number::from_f64)
        else {
            continue;
        };
        *value = Value::Number(number);
    }
}

fn lowercase_hex(value: &mut Value) {
    match value {
        Value::String(text) if is_hex(text) => text.make_ascii_lowercase(),
        Value::Array(items) => items.iter_mut().for_each(lowercase_hex),
        Value::Object(fields) => fields.values_mut().for_each(lowercase_hex),
        _ => {}
    }
}

fn is_hex(text: &str) -> bool {
    text.strip_prefix("0x").is_some_and(hex::check_raw)
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
        alloy_primitives::address,
        model::order::{OrderData, OrderMetadata},
    };

    #[test]
    fn order_view_uses_api_names_numbers_and_lowercase_hex() {
        let order = Order {
            metadata: OrderMetadata {
                owner: address!("0xAbCdEf0000000000000000000000000000000001"),
                full_app_data: Some(r#"{"appCode":"X"}"#.to_owned()),
                ..Default::default()
            },
            data: OrderData {
                sell_amount: "1000000000000000000000".parse().unwrap(),
                ..Default::default()
            },
            ..Default::default()
        };
        let app_data = app_data::document_view(order.metadata.full_app_data.as_deref());
        let view = order_view(&order, app_data);

        assert_eq!(view["owner"], "0xabcdef0000000000000000000000000000000001");
        assert_eq!(view["sellAmount"], 1e21);
        assert_eq!(view["kind"], "buy");
        assert_eq!(view["class"], "limit");
        assert_eq!(view["signingScheme"], "eip712");
        assert_eq!(view["appData"]["appCode"], "X");
        assert!(view["appData"]["metadata"]["flashloan"].is_null());
    }

    #[test]
    fn normalizes_only_hex() {
        assert_eq!(normalize("0xAB"), "0xab");
        assert_eq!(normalize("Acme"), "Acme");
        assert_eq!(normalize("0x"), "0x");
        assert_eq!(normalize("0xzz"), "0xzz");
    }
}
