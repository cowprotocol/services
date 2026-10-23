//! The app-data document as a reader sees it.

use {crate::ProtocolAppData, serde::Serialize, serde_json::Value};

/// The top-level shape of an app-data document, so every key a reader may
/// look up exists even when the document does not set it.
#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    app_code: Option<String>,
    environment: Option<String>,
    version: Option<String>,
    metadata: ProtocolAppData,
}

/// The document as written, with the protocol fields parsed by this crate laid
/// over `metadata`, so every protocol field is present and normalized. A
/// missing or malformed document yields the defaults.
pub fn document_view(full_app_data: Option<&str>) -> Value {
    let mut view = serde_json::to_value(Document::default()).expect("app data serializes");
    let document = full_app_data.and_then(|document| serde_json::from_str::<Value>(document).ok());
    if let Some(document @ Value::Object(_)) = document {
        merge(&mut view, document);
    }
    let protocol = full_app_data
        .and_then(|document| crate::parse(document.as_bytes()).ok())
        .unwrap_or_default();
    merge(
        &mut view["metadata"],
        serde_json::to_value(protocol).expect("app data serializes"),
    );
    view
}

/// Lays `overlay` over `base`: objects merge key by key, anything else is
/// replaced.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn falls_back_to_defaults() {
        let view = document_view(None);
        assert_eq!(document_view(Some("not json")), view);
        assert_eq!(document_view(Some("[1]")), view);
        assert!(view["appCode"].is_null());
        assert!(view.get("backend").is_none());
        assert!(view["metadata"]["signer"].is_null());
        assert!(view["metadata"]["flashloan"].is_null());
        assert!(view["metadata"]["validFrom"].is_null());
        assert_eq!(view["metadata"]["enableFastPath"], false);
        assert_eq!(view["metadata"]["partnerFee"], json!([]));
        assert_eq!(view["metadata"]["wrappers"], json!([]));
        assert_eq!(view["metadata"]["hooks"], json!({ "pre": [], "post": [] }));
    }

    #[test]
    fn keeps_document_fields_and_normalizes_protocol_ones() {
        let view = document_view(Some(
            r#"{
                "appCode": "aave",
                "metadata": {
                    "hooks": { "pre": [{ "target": "0x0202020202020202020202020202020202020202", "callData": "0x", "gasLimit": "1" }] },
                    "partnerFee": { "bps": 10, "recipient": "0x0101010101010101010101010101010101010101" },
                    "custom": { "deep": true }
                }
            }"#,
        ));
        assert_eq!(view["appCode"], "aave");
        assert!(view["environment"].is_null());
        assert_eq!(
            view["metadata"]["hooks"]["pre"].as_array().unwrap().len(),
            1
        );
        assert_eq!(view["metadata"]["hooks"]["post"], json!([]));
        assert_eq!(view["metadata"]["partnerFee"][0]["volumeBps"], 10);
        assert_eq!(view["metadata"]["custom"]["deep"], true);
        assert!(view["metadata"]["flashloan"].is_null());
    }

    #[test]
    fn reads_legacy_backend_hooks() {
        let view = document_view(Some(
            r#"{ "backend": { "hooks": { "post": [{ "target": "0x0202020202020202020202020202020202020202", "callData": "0x", "gasLimit": "1" }] } } }"#,
        ));
        assert_eq!(
            view["metadata"]["hooks"]["post"].as_array().unwrap().len(),
            1
        );
    }

    #[test]
    fn malformed_protocol_fields_fall_back_to_defaults() {
        let view = document_view(Some(
            r#"{ "appCode": "x", "metadata": { "flashloan": "garbage" } }"#,
        ));
        assert_eq!(view["appCode"], "x");
        assert!(view["metadata"]["flashloan"].is_null());
        let view = document_view(Some(r#"{ "appCode": "x", "metadata": 5 }"#));
        assert_eq!(view["appCode"], "x");
        assert_eq!(view["metadata"]["partnerFee"], json!([]));
    }
}
