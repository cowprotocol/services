//! Inbound `/solve` request: the auction the autopilot posts to the driver.
//!
//! This is the driver's own mirror of `autopilot-svm`'s
//! `infra/driver/dto.rs::SolveRequest`. The pinned-literal test below (which
//! uses the same literals as the autopilot's own test) keeps the wire format
//! in sync.

use {
    crate::{
        domain::{self, order_uid::OrderUid},
        infra::api::routes::Kind,
    },
    serde::{Deserialize, Serialize},
    serde_with::{DisplayFromStr, serde_as},
    solana_sdk::pubkey::Pubkey,
    std::{fmt, str::FromStr},
};

/// Application-specific data attached to a Solana order: 32 opaque bytes.
/// Serialized as `0x`-prefixed hex on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppData(pub [u8; 32]);

impl fmt::Display for AppData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = const_hex::Buffer::<32, true>::new();
        f.write_str(buffer.format(&self.0))
    }
}

impl FromStr for AppData {
    type Err = const_hex::FromHexError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut bytes = [0u8; 32];
        const_hex::decode_to_slice(s.strip_prefix("0x").unwrap_or(s), &mut bytes)?;
        Ok(Self(bytes))
    }
}

/// The auction posted to `/solve`.
#[serde_as]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolveRequest {
    /// Autopilot-assigned auction id.
    id: i64,
    /// Timestamp deadline for answering `/solve`.
    deadline: chrono::DateTime<chrono::Utc>,
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
    #[serde_as(as = "DisplayFromStr")]
    app_data: AppData,
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
            app_data: order.app_data.0,
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
    pub fn into_domain(self) -> Result<domain::Auction, Error> {
        let id = domain::auction::Id::try_from(self.id)?;
        Ok(domain::Auction {
            id: Some(id),
            orders: self.orders.into_iter().map(Into::into).collect(),
            // Placeholder; the domain does not consume it yet.
            deadline_slot: domain::Slot(0),
            deadline: self.deadline,
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
            app_data: AppData([0; 32]),
        }
    }

    /// Pins the wire format against the same literals as
    /// `autopilot-svm/src/infra/driver/dto.rs::tests`, so drift between the two
    /// crates breaks a test.
    #[test]
    fn solve_request_pins_the_wire_format() {
        let request = SolveRequest {
            id: 7,
            deadline: "2026-01-01T00:00:00Z".parse().unwrap(),
            orders: vec![order()],
        };
        let expected = serde_json::json!({
            "id": 7,
            "deadline": "2026-01-01T00:00:00Z",
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
                "appData": "0x0000000000000000000000000000000000000000000000000000000000000000",
            }]
        });
        assert_eq!(serde_json::to_value(&request).unwrap(), expected);
    }

    #[test]
    fn into_domain_rejects_non_positive_id() {
        let request = SolveRequest {
            id: 0,
            deadline: "2026-01-01T00:00:00Z".parse().unwrap(),
            orders: vec![order()],
        };
        let err = request
            .into_domain()
            .expect_err("non-positive id must be rejected");
        assert!(matches!(err, Error::InvalidAuctionId));
    }
}
