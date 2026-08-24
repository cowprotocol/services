//! Inbound `/settle` request: asks the driver to submit a previously proposed
//! solution.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SettleRequest`.

use serde::{Deserialize, Serialize};

/// Asks the driver to submit a previously proposed solution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    pub auction_id: i64,
    pub solution_id: u64,
    /// The last slot the settlement transaction may land in.
    pub submission_deadline_slot: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the wire format: all fields are plain integers.
    #[test]
    fn settle_request_pins_the_wire_format() {
        let request = SettleRequest {
            auction_id: 7,
            solution_id: 3,
            submission_deadline_slot: 125,
        };
        let expected = serde_json::json!({
            "auctionId": 7,
            "solutionId": 3,
            "submissionDeadlineSlot": 125
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), expected);
    }
}
