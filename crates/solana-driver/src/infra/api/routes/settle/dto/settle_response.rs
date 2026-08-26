//! Outbound `/settle` response: the transaction signature of the submitted
//! settlement.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SettleResponse`. The autopilot already deserializes
//! this shape.

use {
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::signature::Signature,
};

/// The driver's `/settle` answer.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleResponse {
    /// Transaction signature of the submitted settlement.
    #[serde_as(as = "DisplayFromStr")]
    tx_signature: Signature,
}

impl SettleResponse {
    /// Build a settle response from the submitted transaction signature.
    pub fn new(tx_signature: Signature) -> Self {
        Self { tx_signature }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the wire format against the same literal as
    /// `autopilot-svm/src/infra/driver/dto.rs::tests`.
    #[test]
    fn settle_response_pins_the_wire_format() {
        let settle = SettleResponse {
            tx_signature: Signature::from([9; 64]),
        };
        let expected = serde_json::json!({
            "txSignature": "BUguQsv2ZuHus54HAFzjdJHzZBkygAjKhEeYwSG19tUfUyvvz3worsdQCdAXDNjakJHioSiyxhFiDJrm8XpSXRA"
        });
        assert_eq!(serde_json::to_value(&settle).unwrap(), expected);
    }
}
