//! Wire shape of the order status endpoint.

use {
    database::solana::OrderEventLabel,
    serde::Serialize,
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
};

/// Auction progress of an order.
///
/// TODO: the `value` payloads stay empty until the autopilot persists
/// competition results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", content = "value")]
pub enum Status {
    /// Order is part of the orderbook but not actively being worked on. This
    /// can for example happen if the necessary balances are missing.
    Open,
    /// Order awaits being put into the current auction.
    Scheduled,
    /// Order is part of the current auction and solvers are computing
    /// solutions for it.
    Active,
    /// Some solvers proposed solutions for the orders but did not win the
    /// competition.
    Solved(Vec<SolutionInclusion>),
    /// The order was contained in the winning solution which the solver
    /// currently tries to submit on-chain.
    Executing(Vec<SolutionInclusion>),
    /// The order was successfully executed on-chain.
    Traded(Vec<SolutionInclusion>),
    /// The user cancelled the order. It will no longer show up in any
    /// auctions.
    Cancelled,
}

/// One solution's view of the order.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SolutionInclusion {
    /// The solver that proposed the solution.
    #[serde_as(as = "DisplayFromStr")]
    pub solver: Pubkey,
    /// The amounts the solution executes for the order, absent when the
    /// solution does not include the order.
    pub executed_amounts: Option<ExecutedAmounts>,
}

/// Executed amounts in base units as decimal strings.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutedAmounts {
    #[serde_as(as = "DisplayFromStr")]
    pub sell: u64,
    #[serde_as(as = "DisplayFromStr")]
    pub buy: u64,
}

impl From<OrderEventLabel> for Status {
    fn from(label: OrderEventLabel) -> Self {
        match label {
            OrderEventLabel::Created => Self::Scheduled,
            OrderEventLabel::Ready => Self::Active,
            OrderEventLabel::Considered => Self::Solved(Vec::new()),
            OrderEventLabel::Executing => Self::Executing(Vec::new()),
            OrderEventLabel::Traded => Self::Traded(Vec::new()),
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
            serde_json::to_value(Status::Traded(Vec::new())).unwrap(),
            serde_json::json!({"type": "traded", "value": []})
        );
        assert_eq!(
            serde_json::to_value(Status::Scheduled).unwrap(),
            serde_json::json!({"type": "scheduled"})
        );
        assert_eq!(
            serde_json::to_value(Status::Solved(vec![SolutionInclusion {
                solver: Pubkey::new_from_array([0x22; 32]),
                executed_amounts: Some(ExecutedAmounts { sell: 5, buy: 3 }),
            }]))
            .unwrap(),
            serde_json::json!({"type": "solved", "value": [{
                "solver": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
                "executedAmounts": {"sell": "5", "buy": "3"},
            }]})
        );
    }

    #[test]
    fn labels_map_to_statuses() {
        assert_eq!(Status::from(OrderEventLabel::Created), Status::Scheduled);
        assert_eq!(Status::from(OrderEventLabel::Ready), Status::Active);
        assert_eq!(
            Status::from(OrderEventLabel::Considered),
            Status::Solved(Vec::new())
        );
        assert_eq!(
            Status::from(OrderEventLabel::Executing),
            Status::Executing(Vec::new())
        );
        assert_eq!(
            Status::from(OrderEventLabel::Traded),
            Status::Traded(Vec::new())
        );
        assert_eq!(Status::from(OrderEventLabel::Cancelled), Status::Cancelled);
        assert_eq!(Status::from(OrderEventLabel::Filtered), Status::Open);
        assert_eq!(Status::from(OrderEventLabel::Invalid), Status::Open);
    }
}
