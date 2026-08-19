//! Inbound `/solve` request: the auction the autopilot posts to the driver.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SolveRequest`. The pinned-literal test below (which
//! uses the same literals as the autopilot's own test) keeps the wire format
//! in sync.

use {
    crate::domain::{self, order_uid::OrderUid},
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    std::time::Duration,
};

/// Placeholder solve budget until the deadline is derived from `deadline_slot`.
// TODO: derive the deadline from `deadline_slot` via
// `SolanaRPC::get_block_time`/`get_slot`.
const SOLVE_DEADLINE: Duration = Duration::from_secs(15);

/// The auction posted to `/solve`.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    /// Autopilot-assigned auction id.
    #[serde_as(as = "DisplayFromStr")]
    id: i64,
    /// Slot after which a settlement for this auction is late.
    #[serde_as(as = "DisplayFromStr")]
    deadline_slot: u64,
    orders: Vec<Order>,
}

/// One solvable order in the auction.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    #[serde_as(as = "DisplayFromStr")]
    uid: OrderUid,
    #[serde_as(as = "DisplayFromStr")]
    owner: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    sell_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    buy_token: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    sell_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    buy_token_account: Pubkey,
    #[serde_as(as = "DisplayFromStr")]
    sell_amount: u64,
    #[serde_as(as = "DisplayFromStr")]
    buy_amount: u64,
    /// Unix seconds.
    valid_to: u32,
    kind: Kind,
    partially_fillable: bool,
    #[serde_as(as = "DisplayFromStr")]
    order_pda: Pubkey,
}

/// Whether the order sells or buys an exact amount.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Kind {
    Sell,
    Buy,
}

impl From<Kind> for domain::Side {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Sell => domain::Side::Sell,
            Kind::Buy => domain::Side::Buy,
        }
    }
}

impl From<Order> for domain::Order {
    fn from(order: Order) -> Self {
        Self {
            uid: order.uid,
            owner: order.owner,
            sell_token: order.sell_token,
            buy_token: order.buy_token,
            sell_token_account: order.sell_token_account,
            buy_token_account: order.buy_token_account,
            sell_amount: order.sell_amount,
            buy_amount: order.buy_amount,
            valid_to: order.valid_to,
            side: order.kind.into(),
            partially_fillable: order.partially_fillable,
            order_pda: order.order_pda,
        }
    }
}

/// A `/solve` request that could not become a domain auction.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid auction id")]
    InvalidAuctionId,
}

impl From<domain::auction::InvalidAuctionId> for Error {
    fn from(_: domain::auction::InvalidAuctionId) -> Self {
        Self::InvalidAuctionId
    }
}

impl SolveRequest {
    /// Convert the wire request into a domain auction.
    ///
    /// The driver uses `now + SOLVE_DEADLINE` as a placeholder for the
    /// wall-clock `deadline`.
    pub fn into_domain(self) -> Result<domain::Auction, Error> {
        let id = domain::auction::Id::try_from(self.id)?;
        // TODO: derive the deadline from `deadline_slot` via
        // `SolanaRPC::get_block_time`/`get_slot` once that type adds these methods.
        let deadline = chrono::Utc::now()
            + chrono::Duration::from_std(SOLVE_DEADLINE)
                .expect("solve-deadline fits in a chrono duration");
        Ok(domain::Auction {
            id,
            orders: self.orders.into_iter().map(Into::into).collect(),
            deadline_slot: domain::Slot(self.deadline_slot),
            deadline,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn order() -> Order {
        Order {
            uid: OrderUid([0x11; 32]),
            owner: Pubkey::new_from_array([0x22; 32]),
            sell_token: Pubkey::new_from_array([0x33; 32]),
            buy_token: Pubkey::new_from_array([0x44; 32]),
            sell_token_account: Pubkey::new_from_array([0x55; 32]),
            buy_token_account: Pubkey::new_from_array([0x66; 32]),
            sell_amount: u64::MAX,
            buy_amount: 2_000,
            valid_to: 42,
            kind: Kind::Sell,
            partially_fillable: false,
            order_pda: Pubkey::new_from_array([0x77; 32]),
        }
    }

    /// Pins the wire format against the same literals as
    /// `autopilot-svm/src/infra/driver/dto.rs::tests`, so drift between the two
    /// crates breaks a test.
    #[test]
    fn solve_request_pins_the_wire_format() {
        let request = SolveRequest {
            id: 7,
            deadline_slot: 100,
            orders: vec![order()],
        };
        let expected = serde_json::json!({
            "id": "7",
            "deadlineSlot": "100",
            "orders": [{
                "uid": "0x1111111111111111111111111111111111111111111111111111111111111111",
                "owner": "3JF3sEqM796hk5WFqA6EtmEwJQ9quALszsfJyvXNQKy3",
                "sellToken": pubkey(0x33).to_string(),
                "buyToken": pubkey(0x44).to_string(),
                "sellTokenAccount": pubkey(0x55).to_string(),
                "buyTokenAccount": pubkey(0x66).to_string(),
                "sellAmount": "18446744073709551615",
                "buyAmount": "2000",
                "validTo": 42,
                "kind": "sell",
                "partiallyFillable": false,
                "orderPda": pubkey(0x77).to_string(),
            }]
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), expected);
    }

    #[test]
    fn into_domain_rejects_non_positive_id() {
        let request = SolveRequest {
            id: 0,
            deadline_slot: 100,
            orders: vec![order()],
        };
        let err = request
            .into_domain()
            .expect_err("non-positive id must be rejected");
        assert!(matches!(err, Error::InvalidAuctionId));
    }
}
