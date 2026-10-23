//! Inbound `/settle` request: asks the driver to submit a previously proposed
//! solution.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SettleRequest`.

use {
    serde::{Deserialize, Serialize},
    serde_with::{base64::Base64, serde_as},
};

/// Asks the driver to submit a previously proposed solution.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    pub auction_id: i64,
    pub solution_id: u64,
    /// The last slot the settlement transaction may land in.
    pub submission_deadline_slot: u64,
    /// Fully signed sponsored creation transactions to land before the
    /// settlement, each serialized and base64-encoded.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[serde_as(as = "Vec<Base64>")]
    pub creations: Vec<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the wire format: plain integers, and creations as base64 that an
    /// empty list omits.
    #[test]
    fn settle_request_pins_the_wire_format() {
        let request = SettleRequest {
            auction_id: 7,
            solution_id: 3,
            submission_deadline_slot: 125,
            creations: vec![],
        };
        let expected = serde_json::json!({
            "auctionId": 7,
            "solutionId": 3,
            "submissionDeadlineSlot": 125
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<SettleRequest>(expected).unwrap(),
            request
        );

        let request = SettleRequest {
            creations: vec![vec![1, 2, 3]],
            ..request
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["creations"], serde_json::json!(["AQID"]));
        assert_eq!(
            serde_json::from_value::<SettleRequest>(json).unwrap(),
            request
        );
    }
}
