//! The order as a rule sees it.

mod app_data;

pub use app_data::{AppData, Flashloan, Hook, Hooks, PartnerFee, PartnerFeePolicy, WrapperCall};
use {
    app_data::app_data_view,
    serde_json::{Value, json},
};

/// An order as the host hands it to the engine: the API order plus the facts
/// the host computes for it.
#[derive(Debug, Clone, Copy)]
pub struct Order<'a> {
    pub order: &'a model::order::Order,
    /// The order's app data, which the host copies from the parsed document.
    pub app_data: &'a AppData,
    /// Whether the limit price is inside the market price of the order's quote;
    /// false when the order has no quote. The engine trusts it as given.
    pub in_market: bool,
}

/// The order as rules see it: every field a rule can read, with amounts and
/// prices as doubles and hex in lowercase. Unset optional fields are absent,
/// not `null`. Execution state, settlement and legacy fields are left out.
pub(crate) fn order_view(
    Order {
        order,
        app_data,
        in_market,
    }: Order,
) -> Value {
    let (data, meta) = (&order.data, &order.metadata);
    let mut view = json!({
        "uid": hex(&meta.uid.0),
        "owner": hex(meta.owner.as_slice()),
        "creationDate": meta.creation_date,
        "sellToken": hex(data.sell_token.as_slice()),
        "buyToken": hex(data.buy_token.as_slice()),
        "receiver": hex(data.receiver.raw_bytes()),
        "sellAmount": number(data.sell_amount),
        "buyAmount": number(data.buy_amount),
        "feeAmount": number(data.fee_amount),
        "kind": data.kind,
        "partiallyFillable": data.partially_fillable,
        "sellTokenBalance": data.sell_token_balance,
        "buyTokenBalance": data.buy_token_balance,
        "signingScheme": order.signature.scheme(),
        "validTo": data.valid_to,
        "validFrom": meta.valid_from,
        "appDataHash": hex(&data.app_data.0),
        "appData": app_data_view(app_data),
        "fastPath": meta.fast_path,
        "inMarket": in_market,
        "quote": meta.quote.as_ref().map(|quote| json!({
            "sellAmount": number(quote.sell_amount),
            "buyAmount": number(quote.buy_amount),
            "feeAmount": number(quote.fee_amount),
            "gasAmount": number(quote.gas_amount.to_plain_string()),
            "gasPrice": number(quote.gas_price.to_plain_string()),
            "sellTokenPrice": number(quote.sell_token_price.to_plain_string()),
            "solver": hex(quote.solver.as_slice()),
            "verified": quote.verified,
        })),
        "ethflowData": meta.ethflow_data.as_ref().map(|ethflow| json!({
            "userValidTo": ethflow.user_valid_to,
            "refundTxHash": ethflow.refund_tx_hash.map(|hash| hex(hash.as_slice())),
            "isRefunded": ethflow.refund_tx_hash.is_some(),
        })),
        "onchainOrderData": meta.onchain_order_data.as_ref().map(|onchain| json!({
            "sender": hex(onchain.sender.as_slice()),
            "placementError": onchain.placement_error,
        })),
        "onchainUser": meta.onchain_user.map(|user| hex(user.as_slice())),
    });
    remove_nulls(&mut view);
    view
}

fn hex(bytes: &[u8]) -> String {
    const_hex::encode_prefixed(bytes)
}

/// A decimal amount or price as a double, rounded once from its plain text.
fn number(decimal: impl ToString) -> f64 {
    decimal
        .to_string()
        .parse()
        .expect("decimals parse as doubles")
}

/// Drops `null` fields, so that an unset option is absent, as `has()` expects.
fn remove_nulls(value: &mut Value) {
    if let Value::Object(fields) = value {
        fields.retain(|_, field| !field.is_null());
        fields.values_mut().for_each(remove_nulls);
    }
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
        alloy_primitives::{B256, U256, address},
        model::order::{
            EthflowData,
            OnchainOrderData,
            OnchainOrderPlacementError,
            OrderData,
            OrderMetadata,
            OrderQuote,
        },
        std::collections::BTreeSet,
    };

    #[test]
    fn order_view_uses_api_names_numbers_and_lowercase_hex() {
        let order = model::order::Order {
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
        let view = order_view(Order {
            order: &order,
            app_data: &Default::default(),
            in_market: false,
        });

        assert_eq!(view["owner"], "0xabcdef0000000000000000000000000000000001");
        assert_eq!(view["sellAmount"], 1e21);
        assert_eq!(view["kind"], "buy");
        assert_eq!(view["inMarket"], false);
        assert!(view.get("class").is_none());
        assert_eq!(view["signingScheme"], "eip712");
        assert_eq!(view["appDataHash"], format!("0x{}", "0".repeat(64)));
        assert_eq!(view["fastPath"], false);
        assert!(view.get("quote").is_none());
        assert_eq!(view["appData"]["metadata"]["partnerFee"], json!([]));
    }

    /// An order with every option set, the view's fields at their fullest.
    fn fully_set_order() -> model::order::Order {
        model::order::Order {
            metadata: OrderMetadata {
                owner: address!("0xAbCdEf0000000000000000000000000000000001"),
                quote: Some(OrderQuote {
                    sell_amount: U256::from(10).pow(U256::from(21)),
                    gas_amount: "21000".parse().unwrap(),
                    gas_price: "1234.5".parse().unwrap(),
                    sell_token_price: "0.0000001".parse().unwrap(),
                    solver: address!("0x00000000000000000000000000000000000000aB"),
                    verified: true,
                    ..Default::default()
                }),
                available_balance: Some(U256::from(7)),
                gas_cost: Some(U256::from(8)),
                executed_sell_amount: 9u64.into(),
                ethflow_data: Some(EthflowData {
                    user_valid_to: 5,
                    refund_tx_hash: Some(B256::repeat_byte(0xab)),
                }),
                onchain_order_data: Some(OnchainOrderData {
                    sender: address!("0x00000000000000000000000000000000000000Cd"),
                    placement_error: Some(OnchainOrderPlacementError::ValidToTooFarInTheFuture),
                }),
                onchain_user: Some(address!("0x00000000000000000000000000000000000000Ef")),
                valid_from: Some(4),
                fast_path: true,
                ..Default::default()
            },
            data: OrderData {
                sell_token: address!("0x00000000000000000000000000000000000000aA"),
                receiver: address!("0x00000000000000000000000000000000000000bB").into(),
                sell_amount: U256::from(10).pow(U256::from(21)),
                buy_amount: U256::from(123_456_789),
                partially_fillable: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// Fails unless `view` equals `api` up to the view's two conversions:
    /// decimal strings become doubles and hex strings become lowercase.
    fn assert_matches_api(view: &Value, api: &Value, path: &str) {
        match (view, api) {
            (Value::Object(fields), _) => {
                for (name, field) in fields {
                    assert_matches_api(field, &api[name], &format!("{path}.{name}"));
                }
            }
            (Value::Number(number), Value::String(text)) => {
                assert_eq!(number.as_f64(), text.parse().ok(), "{path}");
            }
            (_, Value::String(text)) => assert_eq!(*view, normalize(text), "{path}"),
            _ => assert_eq!(view, api, "{path}"),
        }
    }

    /// Every view field is the API field of the same name, so a rule author
    /// can read the API docs. The API lacks `inMarket`, and its order calls
    /// the hash `appData` and has the document as `fullAppData` text.
    #[test]
    fn view_matches_the_api_json() {
        let order = fully_set_order();
        let mut view = order_view(Order {
            order: &order,
            app_data: &crate::rules::sample_app_data(),
            in_market: true,
        });
        let fields = view.as_object_mut().unwrap();
        fields.remove("inMarket");
        let hash = fields.remove("appDataHash").unwrap();
        fields.insert("appData".to_owned(), hash);
        assert_matches_api(&view, &serde_json::to_value(&order).unwrap(), "");
    }

    /// Every field of `view` as a path, nested objects and list items
    /// included, e.g. `.quote.sellAmount` or
    /// `.appData.metadata.wrappers[]`.
    fn field_paths(view: &Value) -> BTreeSet<String> {
        let children: Vec<(String, &Value)> = match view {
            Value::Object(fields) => fields
                .iter()
                .map(|(name, field)| (format!(".{name}"), field))
                .collect(),
            Value::Array(items) => items.iter().map(|item| ("[]".to_owned(), item)).collect(),
            _ => Vec::new(),
        };
        let mut paths = BTreeSet::new();
        for (step, child) in children {
            paths.extend(
                field_paths(child)
                    .into_iter()
                    .map(|path| format!("{step}{path}")),
            );
            paths.insert(step);
        }
        paths
    }

    /// The view holds the rule fields and nothing else: no execution state,
    /// settlement or legacy field, and unset options are absent.
    #[test]
    fn view_holds_only_rule_fields() {
        let view = order_view(Order {
            order: &fully_set_order(),
            app_data: &crate::rules::sample_app_data(),
            in_market: true,
        });

        let fields: BTreeSet<&str> = view
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            fields,
            BTreeSet::from([
                "uid",
                "owner",
                "creationDate",
                "sellToken",
                "buyToken",
                "receiver",
                "sellAmount",
                "buyAmount",
                "feeAmount",
                "kind",
                "partiallyFillable",
                "sellTokenBalance",
                "buyTokenBalance",
                "signingScheme",
                "validTo",
                "validFrom",
                "appDataHash",
                "appData",
                "fastPath",
                "inMarket",
                "quote",
                "ethflowData",
                "onchainOrderData",
                "onchainUser",
            ])
        );
        assert_eq!(view["quote"]["gasPrice"], 1234.5);
        assert_eq!(view["quote"]["sellTokenPrice"], 1e-7);
        assert_eq!(view["quote"]["sellAmount"], 1e21);
        assert_eq!(
            view["ethflowData"],
            json!({
                "userValidTo": 5,
                "refundTxHash": format!("0x{}", "ab".repeat(32)),
                "isRefunded": true,
            })
        );
        assert_eq!(
            view["onchainOrderData"],
            json!({
                "sender": format!("0x{}cd", "0".repeat(38)),
                "placementError": "validToTooFarInTheFuture",
            })
        );
        assert_eq!(view["onchainUser"], format!("0x{}ef", "0".repeat(38)));
        assert_eq!(view["validFrom"], 4);
    }

    /// The sample order that validates rules reaches every field, nested ones
    /// included, so that no valid rule fails on a missing key.
    #[test]
    fn sample_order_view_has_every_field() {
        let view = |order| {
            order_view(Order {
                order,
                app_data: &crate::rules::sample_app_data(),
                in_market: false,
            })
        };
        assert_eq!(
            field_paths(&view(&crate::rules::sample_order())),
            field_paths(&view(&fully_set_order())),
        );
    }

    #[test]
    fn unset_nested_options_are_absent() {
        let order = model::order::Order {
            metadata: OrderMetadata {
                ethflow_data: Some(EthflowData::default()),
                onchain_order_data: Some(OnchainOrderData::default()),
                ..Default::default()
            },
            ..Default::default()
        };
        let view = order_view(Order {
            order: &order,
            app_data: &Default::default(),
            in_market: false,
        });

        assert_eq!(
            view["ethflowData"],
            json!({ "userValidTo": 0, "isRefunded": false })
        );
        assert_eq!(
            view["onchainOrderData"],
            json!({ "sender": format!("0x{}", "0".repeat(40)) })
        );
    }

    /// The README's sample order has every field of the view, no more.
    #[test]
    fn readme_sample_has_every_field() {
        let readme = include_str!("../../README.md");
        let section = &readme[readme.find("### `order`").unwrap()..];
        let sample = section.split("```json\n").nth(1).unwrap();
        let sample: Value = serde_json::from_str(sample.split("```").next().unwrap()).unwrap();
        let view = order_view(Order {
            order: &fully_set_order(),
            app_data: &crate::rules::sample_app_data(),
            in_market: true,
        });
        assert_eq!(field_paths(&sample), field_paths(&view));
    }

    #[test]
    fn normalizes_only_hex() {
        assert_eq!(normalize("0xAB"), "0xab");
        assert_eq!(normalize("Acme"), "Acme");
        assert_eq!(normalize("0x"), "0x");
        assert_eq!(normalize("0xzz"), "0xzz");
    }
}
