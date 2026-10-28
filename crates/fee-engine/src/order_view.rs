//! The order as a rule sees it.

use serde_json::{Value, json};

/// An order as the host hands it to the engine: the API order plus the facts
/// the host computes for it.
#[derive(Debug, Clone, Copy)]
pub struct Order<'a> {
    pub order: &'a model::order::Order,
    /// Whether the limit price is inside the market price of the order's quote;
    /// false when the order has no quote.
    pub in_market: bool,
}

/// The order as rules see it: every field a rule can read, with amounts and
/// prices as doubles and hex in lowercase. Unset optional fields are absent,
/// not `null`. Execution state, settlement and legacy fields are left out.
pub(crate) fn order_view(Order { order, in_market }: Order) -> Value {
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
        "appData": hex(&data.app_data.0),
        "fullAppData": meta.full_app_data,
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
            in_market: false,
        });

        assert_eq!(view["owner"], "0xabcdef0000000000000000000000000000000001");
        assert_eq!(view["sellAmount"], 1e21);
        assert_eq!(view["kind"], "buy");
        assert_eq!(view["inMarket"], false);
        assert!(view.get("class").is_none());
        assert_eq!(view["signingScheme"], "eip712");
        assert_eq!(view["appData"], format!("0x{}", "0".repeat(64)));
        assert_eq!(view["fastPath"], false);
        assert!(view.get("quote").is_none());
        assert!(view.get("fullAppData").is_none());
    }

    /// The view holds the rule fields and nothing else: no execution state,
    /// settlement or legacy field, and unset options are absent.
    #[test]
    fn view_holds_only_rule_fields() {
        let order = model::order::Order {
            metadata: OrderMetadata {
                quote: Some(OrderQuote {
                    gas_price: "1234.5".parse().unwrap(),
                    sell_token_price: "0.0000001".parse().unwrap(),
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
                full_app_data: Some("123".to_owned()),
                ..Default::default()
            },
            ..Default::default()
        };
        let view = order_view(Order {
            order: &order,
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
                "appData",
                "fullAppData",
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
        assert_eq!(view["quote"]["sellAmount"], 0.0);
        assert_eq!(view["fullAppData"], "123");
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

    #[test]
    fn normalizes_only_hex() {
        assert_eq!(normalize("0xAB"), "0xab");
        assert_eq!(normalize("Acme"), "Acme");
        assert_eq!(normalize("0x"), "0x");
        assert_eq!(normalize("0xzz"), "0xzz");
    }
}
