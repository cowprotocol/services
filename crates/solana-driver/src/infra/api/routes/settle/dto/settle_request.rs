//! Inbound `/settle` request: asks the driver to submit a previously proposed
//! solution.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SettleRequest`.

use {
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
};

/// Asks the driver to submit a previously proposed solution.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettleRequest {
    #[serde_as(as = "DisplayFromStr")]
    pub auction_id: i64,
    #[serde_as(as = "DisplayFromStr")]
    pub solution_id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pins the wire format: both fields are decimal strings. This matches
    /// the EVM driver's convention of writing integer ids as strings.
    #[test]
    fn settle_request_pins_the_wire_format() {
        let request = SettleRequest {
            auction_id: 7,
            solution_id: 3,
        };
        let expected = serde_json::json!({
            "auctionId": "7",
            "solutionId": "3"
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), expected);
    }
}
