//! Wire shape of the order status endpoint.

use serde::Serialize;

/// Auction progress of an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Status {
    /// Known to the orderbook but not part of a running auction.
    Open,
    /// Awaiting the next auction.
    Scheduled,
    /// Part of the current auction, solvers are working on it.
    Active,
    /// Solutions were proposed but did not win.
    Solved,
    /// Part of the winning solution being submitted on-chain.
    Executing,
    /// Executed on-chain.
    Traded,
    /// Cancelled, no longer entering auctions.
    Cancelled,
}

impl Status {
    /// Map an order-event label to the wire status. The label set is pinned
    /// by the DB enum, a new variant must fail loud.
    pub fn from_label(label: &str) -> Self {
        match label {
            "created" => Self::Scheduled,
            "ready" => Self::Active,
            "considered" => Self::Solved,
            "executing" => Self::Executing,
            "traded" => Self::Traded,
            "cancelled" => Self::Cancelled,
            "filtered" | "invalid" => Self::Open,
            other => unreachable!("unknown order event label {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_format_is_stable() {
        assert_eq!(
            serde_json::to_value(Status::Traded).unwrap(),
            serde_json::json!({"type": "traded"})
        );
        assert_eq!(
            serde_json::to_value(Status::Scheduled).unwrap(),
            serde_json::json!({"type": "scheduled"})
        );
    }

    #[test]
    fn labels_map_to_statuses() {
        assert_eq!(Status::from_label("created"), Status::Scheduled);
        assert_eq!(Status::from_label("ready"), Status::Active);
        assert_eq!(Status::from_label("considered"), Status::Solved);
        assert_eq!(Status::from_label("executing"), Status::Executing);
        assert_eq!(Status::from_label("traded"), Status::Traded);
        assert_eq!(Status::from_label("cancelled"), Status::Cancelled);
        assert_eq!(Status::from_label("filtered"), Status::Open);
        assert_eq!(Status::from_label("invalid"), Status::Open);
    }
}
