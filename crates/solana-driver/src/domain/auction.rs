//! Domain model of an auction the driver asks solver engines to fill.

use {
    super::{order_uid::OrderUid, slot::Slot},
    serde::Serialize,
    solana_sdk::pubkey::Pubkey,
    std::fmt,
};

/// The autopilot-assigned identifier of an auction.
///
/// The id must be positive. The autopilot assigns positive ids, and the
/// engine boundary later reads the id as `u64`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Id(i64);

impl Id {
    /// Construct a validated auction id. Reject non-positive values.
    pub fn new(id: i64) -> Result<Self, InvalidAuctionId> {
        if id <= 0 {
            return Err(InvalidAuctionId(id));
        }
        Ok(Self(id))
    }

    /// The raw id value. Guaranteed positive by construction.
    pub fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for Id {
    type Error = InvalidAuctionId;

    fn try_from(id: i64) -> Result<Self, Self::Error> {
        Self::new(id)
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A non-positive auction id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("auction id must be positive, got {0}")]
pub struct InvalidAuctionId(pub i64);

/// A collection of orders the driver wants solvers to fill.
#[derive(Clone, Debug)]
pub struct Auction {
    pub id: Id,
    pub orders: Vec<Order>,
    /// Slot after which a settlement for this auction is late.
    pub deadline_slot: Slot,
    /// Absolute deadline by which solver engines must return solutions. The
    /// driver derives each request's timeout as the time left until this
    /// instant. It skips the request if the deadline has passed.
    pub deadline: chrono::DateTime<chrono::Utc>,
}

/// One order available for solvers to fill.
#[derive(Clone, Debug)]
pub struct Order {
    pub uid: OrderUid,
    pub owner: Pubkey,
    pub sell_token: Pubkey,
    pub buy_token: Pubkey,
    pub sell_token_account: Pubkey,
    pub buy_token_account: Pubkey,
    pub sell_amount: u64,
    pub buy_amount: u64,
    /// Unix seconds.
    pub valid_to: u32,
    pub side: Side,
    pub partially_fillable: bool,
    pub order_pda: Pubkey,
}

/// Direction of the trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Sell,
    Buy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_accepts_positive_values() {
        assert_eq!(Id::new(1).unwrap(), Id(1));
        assert_eq!(Id::try_from(42).unwrap(), Id(42));
    }

    #[test]
    fn id_rejects_non_positive_values() {
        for id in [0, -1, i64::MIN] {
            assert_eq!(Id::new(id).unwrap_err(), InvalidAuctionId(id));
        }
    }
}
