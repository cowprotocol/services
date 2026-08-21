//! Wire shape of the order status endpoint.

use {database::order_events::OrderEventLabel, serde::Serialize};

/// Auction progress of an order.
///
/// TODO: `solved`, `executing`, and `traded` gain a `value` payload with
/// per-solution solver data (the EVM orderbook's `SolutionInclusion` shape)
/// once the autopilot persists competition results.
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

impl From<OrderEventLabel> for Status {
    fn from(label: OrderEventLabel) -> Self {
        match label {
            OrderEventLabel::Created => Self::Scheduled,
            OrderEventLabel::Ready => Self::Active,
            OrderEventLabel::Considered => Self::Solved,
            OrderEventLabel::Executing => Self::Executing,
            OrderEventLabel::Traded => Self::Traded,
            OrderEventLabel::Cancelled => Self::Cancelled,
            OrderEventLabel::Filtered | OrderEventLabel::Invalid => Self::Open,
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
        assert_eq!(Status::from(OrderEventLabel::Created), Status::Scheduled);
        assert_eq!(Status::from(OrderEventLabel::Ready), Status::Active);
        assert_eq!(Status::from(OrderEventLabel::Considered), Status::Solved);
        assert_eq!(Status::from(OrderEventLabel::Executing), Status::Executing);
        assert_eq!(Status::from(OrderEventLabel::Traded), Status::Traded);
        assert_eq!(Status::from(OrderEventLabel::Cancelled), Status::Cancelled);
        assert_eq!(Status::from(OrderEventLabel::Filtered), Status::Open);
        assert_eq!(Status::from(OrderEventLabel::Invalid), Status::Open);
    }
}
