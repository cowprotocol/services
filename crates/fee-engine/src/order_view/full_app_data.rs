//! The order's app-data document as a rule sees it.

use {
    super::{hex, number},
    app_data::{FeePolicy, Hook, PartnerFee, ProtocolAppData},
    serde_json::{Map, Value, json},
};

/// Root keys the app-data schema defines as strings.
const ROOT_STRINGS: [&str; 3] = ["appCode", "environment", "version"];

/// The document with its protocol fields typed like the rest of the view and
/// every other key as written; the empty document's view when there is no
/// document or it does not parse.
pub(super) fn full_app_data(document: Option<&str>) -> Value {
    document
        .and_then(document_view)
        .unwrap_or_else(|| json!({ "metadata": protocol_fields(&ProtocolAppData::default()) }))
}

fn document_view(document: &str) -> Option<Value> {
    let Value::Object(mut root) = serde_json::from_str(document).ok()? else {
        return None;
    };
    let protocol = app_data::parse(document.as_bytes()).ok()?;
    // Deprecated home of the hooks; `protocol` has them when there is no
    // `metadata`.
    root.remove("backend");
    root.retain(|key, value| !ROOT_STRINGS.contains(&key.as_str()) || value.is_string());
    let mut metadata = match root.remove("metadata") {
        Some(Value::Object(metadata)) => metadata,
        _ => Map::new(),
    };
    // The order's own `fastPath` and `validFrom`.
    metadata.remove("enableFastPath");
    metadata.remove("validFrom");
    // Replaces the protocol keys as written.
    metadata.extend(protocol_fields(&protocol));
    root.insert("metadata".to_owned(), metadata.into());
    Some(root.into())
}

fn protocol_fields(protocol: &ProtocolAppData) -> Map<String, Value> {
    let Value::Object(fields) = json!({
        "hooks": {
            "pre": protocol.hooks.pre.iter().map(hook).collect::<Vec<_>>(),
            "post": protocol.hooks.post.iter().map(hook).collect::<Vec<_>>(),
        },
        "signer": protocol.signer.map(|signer| hex(signer.as_slice())),
        "replacedOrder": protocol.replaced_order.as_ref().map(|replaced| json!({
            "uid": hex(&replaced.uid.0),
        })),
        "partnerFee": protocol.partner_fee.iter().map(partner_fee).collect::<Vec<_>>(),
        "flashloan": protocol.flashloan.as_ref().map(|flashloan| json!({
            "liquidityProvider": hex(flashloan.liquidity_provider.as_slice()),
            "protocolAdapter": hex(flashloan.protocol_adapter.as_slice()),
            "receiver": hex(flashloan.receiver.as_slice()),
            "token": hex(flashloan.token.as_slice()),
            "amount": number(flashloan.amount),
        })),
        "wrappers": protocol.wrappers.iter().map(|wrapper| json!({
            "address": hex(wrapper.address.as_slice()),
            "data": hex(&wrapper.data),
            "isOmittable": wrapper.is_omittable,
        })).collect::<Vec<_>>(),
    }) else {
        unreachable!("a JSON object literal");
    };
    fields
}

fn hook(hook: &Hook) -> Value {
    json!({
        "target": hex(hook.target.as_slice()),
        "callData": hex(&hook.call_data),
        "gasLimit": hook.gas_limit,
    })
}

/// One partner fee under the schema's names; a bare `bps` reads as
/// `volumeBps`, as the protocol reads it.
fn partner_fee(fee: &PartnerFee) -> Value {
    let mut view = match fee.policy {
        FeePolicy::Volume { bps } => json!({ "volumeBps": bps }),
        FeePolicy::Surplus {
            bps,
            max_volume_bps,
        } => json!({ "surplusBps": bps, "maxVolumeBps": max_volume_bps }),
        FeePolicy::PriceImprovement {
            bps,
            max_volume_bps,
        } => json!({ "priceImprovementBps": bps, "maxVolumeBps": max_volume_bps }),
    };
    view["recipient"] = hex(fee.recipient.as_slice()).into();
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The view as the order view holds it: no `null` fields.
    fn view(document: &str) -> Value {
        let mut view = full_app_data(Some(document));
        crate::order_view::remove_nulls(&mut view);
        view
    }

    #[test]
    fn types_protocol_fields_and_keeps_the_rest_as_written() {
        let view = view(
            r#"{
                "appCode": "CoW Swap",
                "environment": 7,
                "metadata": {
                    "quote": { "slippageBips": 50, "smartSlippage": true },
                    "utm": { "utmSource": "0xAB" },
                    "hooks": { "pre": [{
                        "target": "0x00000000000000000000000000000000000000Aa",
                        "callData": "0xAB",
                        "gasLimit": "21000",
                        "dappId": "cow-sdk://x"
                    }] },
                    "partnerFee": [
                        { "bps": 25, "recipient": "0x00000000000000000000000000000000000000Bb" },
                        { "surplusBps": 50, "maxVolumeBps": 100, "recipient": "0x00000000000000000000000000000000000000Bb" },
                        { "priceImprovementBps": 30, "maxVolumeBps": 90, "recipient": "0x00000000000000000000000000000000000000Bb" }
                    ],
                    "flashloan": {
                        "liquidityProvider": "0x00000000000000000000000000000000000000Cc",
                        "protocolAdapter": "0x00000000000000000000000000000000000000Cc",
                        "receiver": "0x00000000000000000000000000000000000000Cc",
                        "token": "0x00000000000000000000000000000000000000Cc",
                        "amount": "6136714"
                    },
                    "enableFastPath": true
                }
            }"#,
        );
        assert_eq!(
            view,
            json!({
                "appCode": "CoW Swap",
                "metadata": {
                    "quote": { "slippageBips": 50, "smartSlippage": true },
                    "utm": { "utmSource": "0xAB" },
                    "hooks": {
                        "pre": [{
                            "target": "0x00000000000000000000000000000000000000aa",
                            "callData": "0xab",
                            "gasLimit": 21000,
                        }],
                        "post": [],
                    },
                    "partnerFee": [
                        { "volumeBps": 25, "recipient": "0x00000000000000000000000000000000000000bb" },
                        { "surplusBps": 50, "maxVolumeBps": 100, "recipient": "0x00000000000000000000000000000000000000bb" },
                        { "priceImprovementBps": 30, "maxVolumeBps": 90, "recipient": "0x00000000000000000000000000000000000000bb" },
                    ],
                    "flashloan": {
                        "liquidityProvider": "0x00000000000000000000000000000000000000cc",
                        "protocolAdapter": "0x00000000000000000000000000000000000000cc",
                        "receiver": "0x00000000000000000000000000000000000000cc",
                        "token": "0x00000000000000000000000000000000000000cc",
                        "amount": 6136714.0,
                    },
                    "wrappers": [],
                },
            })
        );
    }

    #[test]
    fn protocol_fields_from_the_deprecated_backend_key() {
        let view = view(
            r#"{"backend": {"hooks": {"post": [{
                "target": "0x0000000000000000000000000000000000000001",
                "callData": "0x",
                "gasLimit": "1"
            }]}}}"#,
        );
        assert!(view.get("backend").is_none());
        assert_eq!(view["metadata"]["hooks"]["post"][0]["gasLimit"], 1);
    }

    #[test]
    fn no_or_unparsable_document_is_the_empty_document() {
        let empty = full_app_data(Some(app_data::EMPTY));
        assert_eq!(full_app_data(None), empty);
        assert_eq!(full_app_data(Some("not json")), empty);
        assert_eq!(full_app_data(Some("[]")), empty);
        // Both set: the protocol rejects the document.
        assert_eq!(
            full_app_data(Some(
                r#"{"metadata": {"enableFastPath": true, "validFrom": 1}}"#
            )),
            empty
        );

        // The protocol lists are there to read without `has`.
        assert_eq!(
            view(app_data::EMPTY),
            json!({ "metadata": {
                "hooks": { "pre": [], "post": [] },
                "partnerFee": [],
                "wrappers": [],
            } })
        );
    }
}
