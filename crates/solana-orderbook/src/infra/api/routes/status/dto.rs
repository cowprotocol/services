//! Wire shape of the order status endpoint.

use serde::Serialize;

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

impl Status {
    /// Map an order-event label to the wire status. `None` for a label the
    /// mapping does not know, which the label-sweep test turns into a CI
    /// failure whenever a migration extends the DB enum.
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "created" => Some(Self::Scheduled),
            "ready" => Some(Self::Active),
            "considered" => Some(Self::Solved),
            "executing" => Some(Self::Executing),
            "traded" => Some(Self::Traded),
            "cancelled" => Some(Self::Cancelled),
            "filtered" | "invalid" => Some(Self::Open),
            _ => None,
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
        assert_eq!(Status::from_label("created"), Some(Status::Scheduled));
        assert_eq!(Status::from_label("ready"), Some(Status::Active));
        assert_eq!(Status::from_label("considered"), Some(Status::Solved));
        assert_eq!(Status::from_label("executing"), Some(Status::Executing));
        assert_eq!(Status::from_label("traded"), Some(Status::Traded));
        assert_eq!(Status::from_label("cancelled"), Some(Status::Cancelled));
        assert_eq!(Status::from_label("filtered"), Some(Status::Open));
        assert_eq!(Status::from_label("invalid"), Some(Status::Open));
        assert_eq!(Status::from_label("liquidity"), None);
    }

    /// Every label the DB enum can produce has a mapping, so a migration
    /// extending the enum fails here until the mapping learns the variant.
    #[tokio::test]
    #[ignore = "needs the solana.* schema applied to the local database"]
    async fn solana_db_every_event_label_maps() {
        let pool = sqlx::PgPool::connect("postgresql://").await.unwrap();
        let labels: Vec<String> =
            sqlx::query_scalar("SELECT unnest(enum_range(NULL::solana.OrderEventLabel))::text")
                .fetch_all(&pool)
                .await
                .unwrap();
        assert!(!labels.is_empty());
        for label in labels {
            assert!(
                Status::from_label(&label).is_some(),
                "unmapped order event label {label}"
            );
        }
    }
}
