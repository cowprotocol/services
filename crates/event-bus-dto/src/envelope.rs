use {chrono::Utc, schemars::JsonSchema, serde::Serialize};

/// Wire format version of the JSON envelope sent on every event. Bump
/// alongside any breaking change to [`Envelope`].
pub const ENVELOPE_VERSION: &str = "v1";

/// JSON envelope wrapping every event published to the bus. Consumers can
/// rely on `version` to evolve their parsers, on `timestamp` for ordering,
/// and on `requestId` to correlate events emitted while serving a single
/// inbound request (e.g. all the price estimates and the resulting quote of
/// one quote request share the same `requestId`).
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Envelope<T> {
    pub version: &'static str,
    /// RFC3339 timestamp (millisecond precision, UTC) of when the event was
    /// published.
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    pub body: T,
}

impl<T> Envelope<T> {
    pub fn new(request_id: Option<String>, body: T) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            timestamp: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            request_id,
            body,
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, serde_json::json};

    #[test]
    fn envelope_matches_wire_format() {
        let envelope = Envelope::new(Some("req-1".to_string()), json!({"outAmount": 1234}));
        assert_eq!(
            serde_json::to_value(&envelope).unwrap(),
            json!({
                "version": "v1",
                "timestamp": envelope.timestamp,
                "requestId": "req-1",
                "body": {"outAmount": 1234},
            })
        );
    }

    #[test]
    fn envelope_omits_missing_request_id() {
        let envelope = Envelope::new(None, json!({}));
        let serialized = serde_json::to_value(&envelope).unwrap();
        assert!(serialized.get("requestId").is_none());
    }
}
