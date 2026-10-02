//! Domain model of an auction the driver asks solver engines to fill.

use {
    super::{
        buy_token_accounts::{BuyTokenAccountCache, BuyTokenAccounts},
        order_uid::OrderUid,
        slot::Slot,
    },
    crate::infra::blockchain::{Solana, TokenAccountState, associated_token_address},
    cow_settlement_interface::data::intent::ENCODED_NATIVE_SOL_TRANSFER,
    serde::Serialize,
    solana_sdk::{pubkey::Pubkey, transaction::VersionedTransaction},
    std::{collections::HashMap, fmt, sync::Arc},
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
    /// The owner-signed creation transaction of each order not created on
    /// chain yet, by uid. It lacks the funder's signature, so it can only be
    /// simulated ahead of a settlement, never sent.
    pub creations: HashMap<OrderUid, VersionedTransaction>,
}

impl Auction {
    /// Every token buy's buy token account classified against the chain,
    /// resolved once per auction: engines solving the same auction read the
    /// first one's lookup from `cache`.
    pub(super) async fn resolve_buy_token_accounts(
        &self,
        auction_id: Id,
        blockchain: &Solana,
        cache: &BuyTokenAccountCache,
    ) -> Result<Arc<BuyTokenAccounts>, Arc<cow_solana_rpc::Error>> {
        cache
            .resolve(auction_id, self.classify_buy_token_accounts(blockchain))
            .await
    }

    /// Classify every token buy's buy token account, the settlement's payout
    /// destination, against the chain. A native SOL buy pays out to a wallet,
    /// which the payout creates when it is missing, so it needs no lookup.
    async fn classify_buy_token_accounts(
        &self,
        blockchain: &Solana,
    ) -> Result<BuyTokenAccounts, cow_solana_rpc::Error> {
        let token_buys = || self.orders.iter().filter(|order| !order.buys_native_sol());
        let snapshot = blockchain
            .accounts_snapshot(token_buys().map(|order| order.buy_token_account))
            .await?;
        let mut resolved = BuyTokenAccounts::default();
        for order in token_buys() {
            match snapshot.token_account_state(&order.buy_token_account) {
                TokenAccountState::Initialized => (),
                TokenAccountState::NeedsCreation if order.buy_token_account_is_ata() => {
                    resolved.missing.insert(order.uid);
                }
                state => {
                    tracing::warn!(
                        order = %order.uid,
                        buy_token_account = %order.buy_token_account,
                        ?state,
                        "dropping order, its buy token account cannot receive the payout"
                    );
                    resolved.unreceivable.insert(order.uid);
                }
            }
        }
        Ok(resolved)
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
}

impl Order {
    /// Whether `buy_token_account` is the owner's associated token account
    /// for the buy mint, the only destination an idempotent create can
    /// produce.
    pub fn buy_token_account_is_ata(&self) -> bool {
        self.buy_token_account == associated_token_address(&self.owner, &self.buy_token)
    }

    /// Whether the order buys native SOL. The intent encodes it as the System
    /// Program ID in place of a buy mint.
    pub fn buys_native_sol(&self) -> bool {
        self.buy_token == ENCODED_NATIVE_SOL_TRANSFER
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
        cow_solana_rpc::{Mocks, RpcRequest, SolanaRPC},
        serde_json::{Value, json},
        solana_testlib::{multiple_accounts_json, token_account_json},
        std::collections::HashSet,
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
        }
    }

    fn auction(orders: Vec<Order>) -> Auction {
        Auction {
            id: Some(Id(1)),
            orders,
            deadline_slot: Slot(0),
            deadline: chrono::Utc::now(),
            creations: HashMap::new(),
        }
    }

    fn uids(set: &HashSet<OrderUid>) -> Vec<u8> {
        let mut uids: Vec<u8> = set.iter().map(|uid| uid.0[0]).collect();
        uids.sort_unstable();
        uids
    }

    fn blockchain(mocks: Mocks) -> Solana {
        Solana::new(SolanaRPC::new_mock_with_mocks(mocks), pubkey(0xaa))
    }

    /// The lookup answers in order: an initialized token account, absent at
    /// the owner's associated token address, absent elsewhere, and an account
    /// of another program. Only the absent associated token account is the
    /// settlement's to create; the last two can never receive the payout.
    #[tokio::test]
    async fn resolves_each_buy_token_account() {
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

        let auction = auction(vec![
            order(1, pubkey(0x66)),
            order(2, ata),
            order(3, pubkey(0x67)),
            order(4, pubkey(0x68)),
        ]);
        let resolved = auction
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain(mocks),
                &BuyTokenAccountCache::default(),
            )
            .await
            .unwrap();

        assert_eq!(uids(&resolved.missing), [2]);
        assert_eq!(uids(&resolved.unreceivable), [3, 4]);
    }

    /// A native SOL buy pays out to a wallet, so it has no token account to
    /// classify: the lookup answers only for the token buy, absent at the
    /// owner's associated token address.
    #[tokio::test]
    async fn a_native_sol_buy_has_no_buy_token_account_to_resolve() {
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            multiple_accounts_json([Value::Null]),
        )]);
        let native_buy = Order {
            buy_token: ENCODED_NATIVE_SOL_TRANSFER,
            ..order(1, pubkey(0x68))
        };
        let token_buy = order(2, associated_token_address(&pubkey(0x22), &pubkey(0x44)));

        let resolved = auction(vec![native_buy, token_buy])
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain(mocks),
                &BuyTokenAccountCache::default(),
            )
            .await
            .unwrap();

        assert_eq!(uids(&resolved.missing), [2]);
        assert!(resolved.unreceivable.is_empty());
    }

    #[tokio::test]
    async fn fails_when_the_lookup_fails() {
        let mocks = Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            json!("not an account list"),
        )]);
        let auction = auction(vec![order(1, pubkey(0x66))]);

        auction
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain(mocks),
                &BuyTokenAccountCache::default(),
            )
            .await
            .expect_err("a failed lookup fails the resolution");
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
