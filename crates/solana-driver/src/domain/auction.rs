//! Domain model of an auction the driver asks solver engines to fill.

use {
    super::{order_uid::OrderUid, slot::Slot},
    crate::infra::blockchain::{BuyTokenAccountState, Solana},
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
    /// Drop each order whose buy token account can neither receive the payout
    /// nor be created, and record the remaining accounts' state so the engine
    /// can price in the rent of a missing one.
    ///
    /// Fails when the lookup does, or when no order survives.
    pub async fn resolve_buy_token_accounts(
        &mut self,
        blockchain: &Solana,
    ) -> Result<(), ResolveBuyTokenAccountsError> {
        let accounts = self.orders.iter().map(|order| order.buy_token_account);
        let snapshot = blockchain
            .accounts_snapshot(accounts)
            .await
            .map_err(ResolveBuyTokenAccountsError::Rpc)?;
        self.orders.retain_mut(|order| {
            let state = snapshot.buy_token_account_state(order);
            let receivable = state.receivable();
            if !receivable {
                tracing::warn!(
                    order = %order.uid,
                    buy_token_account_address = %order.buy_token_account,
                    buy_token_account_state = ?state,
                    "dropping order, its buy token account cannot receive the payout"
                );
            }
            order.buy_token_account_state = Some(state);
            receivable
        });
        if self.orders.is_empty() {
            return Err(ResolveBuyTokenAccountsError::NoResolvableOrders);
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveBuyTokenAccountsError {
    #[error("rpc request failed: {0}")]
    Rpc(#[source] cow_solana_rpc::Error),
    #[error("no order has a resolvable buy token account")]
    NoResolvableOrders,
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
    /// The on-chain state of `buy_token_account`, `None` until
    /// [`Auction::resolve_buy_token_accounts`] checks it against the chain.
    pub buy_token_account_state: Option<BuyTokenAccountState>,
}

impl Order {
    /// Whether the settlement must create `buy_token_account` before it can
    /// pay out.
    pub fn buy_token_account_missing(&self) -> bool {
        matches!(
            self.buy_token_account_state,
            Some(BuyTokenAccountState::MissingAta)
        )
    }
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
            buy_token_account_state: None,
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
    /// of another program.
    #[tokio::test]
    async fn resolves_missing_atas_and_drops_unreceivable_orders() {
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
            .resolve_buy_token_accounts(&blockchain(mocks))
            .await
            .unwrap();

        let resolved: Vec<(u8, Option<BuyTokenAccountState>)> = auction
            .orders
            .iter()
            .map(|order| (order.uid.0[0], order.buy_token_account_state))
            .collect();
        assert_eq!(
            resolved,
            [
                (1, Some(BuyTokenAccountState::Exists)),
                (2, Some(BuyTokenAccountState::MissingAta)),
            ]
        );
    }

    #[tokio::test]
    async fn fails_when_the_lookup_fails() {
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            json!("not an account list"),
        )]);
        let mut auction = auction(vec![order(1, pubkey(0x66))]);

        let err = auction
            .resolve_buy_token_accounts(&blockchain(mocks))
            .await
            .expect_err("a failed lookup fails the resolution");

        assert!(matches!(err, ResolveBuyTokenAccountsError::Rpc(_)));
        assert!(auction.orders[0].buy_token_account_state.is_none());
    }

    #[tokio::test]
    async fn fails_when_no_order_is_receivable() {
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            multiple_accounts_json([Value::Null]),
        )]);
        // pubkey(0x66) is not the owner's associated token account.
        let mut auction = auction(vec![order(1, pubkey(0x66))]);

        let err = auction
            .resolve_buy_token_accounts(&blockchain(mocks))
            .await
            .expect_err("an auction without a resolvable order fails");

        assert!(matches!(
            err,
            ResolveBuyTokenAccountsError::NoResolvableOrders
        ));
        assert!(auction.orders.is_empty());
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
