//! Domain model of an auction the driver asks solver engines to fill.

use {
    super::{order_uid::OrderUid, slot::Slot},
    crate::infra::blockchain::Solana,
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
    /// `None` when the auction prices a quote instead of a competition.
    pub id: Option<Id>,
    pub orders: Vec<Order>,
    /// Slot after which a settlement for this auction is late.
    pub deadline_slot: Slot,
    /// Absolute deadline by which solver engines must return solutions. The
    /// driver derives each request's timeout as the time left until this
    /// instant. It skips the request if the deadline has passed.
    pub deadline: chrono::DateTime<chrono::Utc>,
}

impl Auction {
    /// Flag each order whose buy token account the settlement will create,
    /// so the engine can price in its rent. A failed lookup flags nothing:
    /// the settlement checks the chain again and creates the account either
    /// way, only the rent goes unpriced.
    pub async fn flag_missing_buy_token_accounts(&mut self, blockchain: &Solana) {
        let accounts = self.orders.iter().map(|order| order.buy_token_account);
        let snapshot = match blockchain.accounts_snapshot(accounts).await {
            Ok(snapshot) => snapshot,
            Err(err) => {
                tracing::warn!(?err, "buy token account lookup failed, flagging no order");
                return;
            }
        };
        for order in &mut self.orders {
            order.missing_buy_token_account = snapshot.buy_token_account_missing(order);
        }
    }
}

/// One order available for solvers to fill.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
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
    pub app_data: [u8; 32],
    /// Whether `buy_token_account` was missing on chain at solve time and
    /// the settlement will create it, see
    /// [`Auction::flag_missing_buy_token_accounts`].
    pub missing_buy_token_account: bool,
}

/// Direction of the trade.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Side {
    Sell,
    Buy,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::infra::blockchain::associated_token_address,
        cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
        serde_json::{Value, json},
        solana_testlib::{multiple_accounts_json, token_account_json},
    };

    fn pubkey(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn order(uid: u8, buy_token_account: Pubkey) -> Order {
        Order {
            uid: OrderUid([uid; 32]),
            owner: pubkey(0x22),
            sell_token: pubkey(0x33),
            buy_token: pubkey(0x44),
            sell_token_account: pubkey(0x55),
            buy_token_account,
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: u32::MAX,
            side: Side::Sell,
            partially_fillable: false,
            order_pda: pubkey(0x77),
            app_data: [0; 32],
            missing_buy_token_account: false,
        }
    }

    fn auction(orders: Vec<Order>) -> Auction {
        Auction {
            id: Some(Id(1)),
            orders,
            deadline_slot: Slot(0),
            deadline: chrono::Utc::now(),
        }
    }

    fn blockchain(mocks: Mocks) -> Solana {
        Solana::new(SolanaRPC::new_mock_with_mocks(mocks), pubkey(0xaa))
    }

    /// The lookup answers in order: an initialized token account, absent at
    /// the owner's associated token address, absent elsewhere, and an account
    /// of another program. Only the absent associated token account is
    /// flagged, and every order stays.
    #[tokio::test]
    async fn flags_only_an_absent_associated_token_account() {
        let ata = associated_token_address(&pubkey(0x22), &pubkey(0x44));
        let foreign = json!({
            "lamports": 1u64,
            "data": ["", "base64"],
            "owner": pubkey(0xff).to_string(),
            "executable": false,
            "rentEpoch": 0u64,
            "space": 0u64,
        });
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            multiple_accounts_json([
                token_account_json(&pubkey(0x44), &pubkey(0x22)),
                Value::Null,
                Value::Null,
                foreign,
            ]),
        )]);

        let mut auction = auction(vec![
            order(1, pubkey(0x66)),
            order(2, ata),
            order(3, pubkey(0x67)),
            order(4, pubkey(0x68)),
        ]);
        auction
            .flag_missing_buy_token_accounts(&blockchain(mocks))
            .await;

        let flagged: Vec<(u8, bool)> = auction
            .orders
            .iter()
            .map(|order| (order.uid.0[0], order.missing_buy_token_account))
            .collect();
        assert_eq!(flagged, [(1, false), (2, true), (3, false), (4, false)]);
    }

    #[tokio::test]
    async fn flags_nothing_when_the_lookup_fails() {
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            json!("not an account list"),
        )]);
        let ata = associated_token_address(&pubkey(0x22), &pubkey(0x44));
        let mut auction = auction(vec![order(1, ata)]);

        auction
            .flag_missing_buy_token_accounts(&blockchain(mocks))
            .await;

        assert!(!auction.orders[0].missing_buy_token_account);
    }

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
