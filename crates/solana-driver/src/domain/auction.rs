//! Domain model of an auction the driver asks solver engines to fill.

use {
    super::{
        buy_token_accounts::{BuyTokenAccountCache, BuyTokenAccounts},
        order_uid::OrderUid,
        slot::Slot,
    },
    crate::infra::blockchain::{
        InvalidMintReason,
        Solana,
        TokenAccountState,
        associated_token_address,
    },
    cow_settlement_interface::{
        data::intent::ENCODED_NATIVE_SOL_TRANSFER,
        token_program::TokenProgram,
    },
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
    /// instant. It skips the request if the deadline has passed. A solve
    /// narrows the autopilot's deadline to the engine's share of it.
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
    ///
    /// An order whose buy mint is missing or not a mint is unreceivable: no
    /// settlement can pay it out.
    async fn classify_buy_token_accounts(
        &self,
        blockchain: &Solana,
    ) -> Result<BuyTokenAccounts, cow_solana_rpc::Error> {
        let token_buys = || self.orders.iter().filter(|order| !order.buys_native_sol());
        let programs = blockchain
            .token_programs(token_buys().map(|order| order.buy_token))
            .await?;
        let snapshot = blockchain
            .accounts_snapshot(token_buys().map(|order| order.buy_token_account))
            .await?;
        let mut resolved = BuyTokenAccounts::default();
        for order in token_buys() {
            // Every token buy's mint was fetched, so a missing entry only
            // guards a bug and drops the order like an invalid mint.
            let program = programs
                .get(&order.buy_token)
                .copied()
                .unwrap_or(Err(InvalidMintReason::AccountNotFound));
            let program = match program {
                Ok(program) => program,
                Err(reason) => {
                    tracing::warn!(
                        order = %order.uid,
                        buy_token = %order.buy_token,
                        %reason,
                        "dropping order, its buy mint cannot be paid out"
                    );
                    resolved.unreceivable.insert(order.uid);
                    continue;
                }
            };
            match snapshot.token_account_state(&order.buy_token_account) {
                TokenAccountState::Initialized => (),
                TokenAccountState::NeedsCreation if order.buy_token_account_is_ata(program) => {
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

    /// Drop the creations whose order PDA already exists on chain: an earlier
    /// settlement attempt landed them and the autopilot still ships them
    /// until the indexer catches up. Simulating one fails on the existing
    /// account and would veto a settlement that no longer needs it. A failed
    /// lookup keeps every creation and leaves the verdict to the simulation.
    pub(super) async fn drop_landed_creations(&mut self, blockchain: &Solana) {
        if self.creations.is_empty() {
            return;
        }
        let pdas = self
            .orders
            .iter()
            .filter(|order| self.creations.contains_key(&order.uid))
            .map(|order| order.order_pda);
        let snapshot = match blockchain.accounts_snapshot(pdas).await {
            Ok(snapshot) => snapshot,
            Err(err) => {
                tracing::warn!(?err, "could not check which creations already landed");
                return;
            }
        };
        for order in &self.orders {
            if snapshot.exists(&order.order_pda) && self.creations.remove(&order.uid).is_some() {
                tracing::info!(order_uid = %order.uid, "creation already landed, settling without it");
            }
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
    /// The cumulative fill on the order's own side: sell-token units for a
    /// sell order, buy-token units for a buy order.
    pub executed: u64,
    /// What the owner's sell token account can fund, as the autopilot read
    /// it at the cut. `None` when unknown: a pending sponsored order, a
    /// quote, an autopilot that predates balance scaling.
    pub sell_balance: Option<u64>,
}

impl Order {
    /// The amounts still open to fill: the order-side target less `executed`,
    /// the other leg scaled in proportion.
    ///
    /// TODO: duplicate of `autopilot-svm`'s `Order::remaining`; unify the two.
    pub fn remaining(&self) -> Remaining {
        let (target, _) = self.legs();
        self.scaled(target.saturating_sub(self.executed))
    }

    /// What a solver may fill now: `remaining` scaled down to what the sell
    /// token account can fund, like the EVM driver's `Order::available`. A
    /// fill-or-kill order cannot shrink, so it keeps its remainder; the
    /// autopilot drops it when the account cannot fund that.
    pub fn available(&self) -> Remaining {
        let remaining = self.remaining();
        let Some(balance) = self.sell_balance else {
            return remaining;
        };
        if !self.partially_fillable || balance >= remaining.sell {
            return remaining;
        }
        let open = match self.side {
            Side::Sell => balance,
            // The largest buy whose sell leg, rounded down, fits the balance.
            // `balance < sell_amount`, so the quotient fits u64.
            Side::Buy => fits(
                u128::from(self.buy_amount) * u128::from(balance) / u128::from(self.sell_amount),
            ),
        };
        self.scaled(open)
    }

    /// The signed order-side target and the other leg.
    fn legs(&self) -> (u64, u64) {
        match self.side {
            Side::Sell => (self.sell_amount, self.buy_amount),
            Side::Buy => (self.buy_amount, self.sell_amount),
        }
    }

    /// The legs for `open` of the order-side target, the other leg scaled in
    /// proportion. Rounds like the EVM driver, the sell leg down and the buy
    /// leg up, so the scaled limit is never looser than the signed one.
    fn scaled(&self, open: u64) -> Remaining {
        if open == 0 {
            return Remaining { sell: 0, buy: 0 };
        }
        let (target, other) = self.legs();
        let scaled = u128::from(other) * u128::from(open);
        let target = u128::from(target);
        // `open <= target`, so the quotient never exceeds `other`.
        match self.side {
            Side::Sell => Remaining {
                sell: open,
                buy: fits(scaled.div_ceil(target)),
            },
            Side::Buy => Remaining {
                sell: fits(scaled / target),
                buy: open,
            },
        }
    }

    /// Whether `buy_token_account` is the owner's associated token account
    /// for the buy mint under the mint's token `program`, the only
    /// destination an idempotent create can produce.
    pub fn buy_token_account_is_ata(&self, program: TokenProgram) -> bool {
        self.buy_token_account == associated_token_address(&self.owner, &self.buy_token, program)
    }

    /// Whether the order buys native SOL. The intent encodes it as the System
    /// Program ID in place of a buy mint.
    pub fn buys_native_sol(&self) -> bool {
        self.buy_token == ENCODED_NATIVE_SOL_TRANSFER
    }
}

/// Narrows a leg scaled from, and bounded by, `u64` amounts.
fn fits(leg: u128) -> u64 {
    u64::try_from(leg).expect("a scaled leg fits u64")
}

/// What is left of an order to fill, see [`Order::remaining`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Remaining {
    pub sell: u64,
    pub buy: u64,
}

impl Remaining {
    /// Whether a leg scaled down to nothing. The program only accepts a fill
    /// moving zero on that side, so no engine can fill the order.
    pub fn has_zero_leg(self) -> bool {
        self.sell == 0 || self.buy == 0
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
        cow_solana_rpc::{MocksMap, RpcRequest, SolanaRPC},
        serde_json::{Value, json},
        solana_testlib::{
            account_json,
            mint_account_json,
            multiple_accounts_json,
            token_2022_mint,
            token_account_json,
        },
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
            executed: 0,
            sell_balance: None,
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

    /// Answers the classification's two `getMultipleAccounts` lookups in
    /// order: `mints` for the token buys' buy mints, then `accounts` for their
    /// buy token accounts, each in auction order.
    fn blockchain(
        mints: impl IntoIterator<Item = Value>,
        accounts: impl IntoIterator<Item = Value>,
    ) -> Solana {
        let mocks = MocksMap::from_iter([
            (
                RpcRequest::GetMultipleAccounts,
                multiple_accounts_json(mints),
            ),
            (
                RpcRequest::GetMultipleAccounts,
                multiple_accounts_json(accounts),
            ),
        ]);
        blockchain_with_mocks_map(mocks)
    }

    fn blockchain_with_mocks_map(mocks: MocksMap) -> Solana {
        Solana::new(
            SolanaRPC::new_mock_with_mocks_map(mocks.clone()),
            SolanaRPC::new_mock_with_mocks_map(mocks),
            pubkey(0xaa),
        )
    }

    /// The buy mint is an SPL Token mint. The account lookup answers in
    /// order: an initialized token account, absent at the owner's associated
    /// token address, absent elsewhere, and an account of another program.
    /// Only the absent associated token account is the settlement's to
    /// create. The last two can never receive the payout.
    #[tokio::test]
    async fn resolves_each_buy_token_account() {
        let ata = associated_token_address(&pubkey(0x22), &pubkey(0x44), TokenProgram::SplToken);
        let foreign = json!({
            "lamports": 1u64,
            "data": ["", "base64"],
            "owner": pubkey(0xff).to_string(),
            "executable": false,
            "rentEpoch": 0u64,
            "space": 0u64,
        });
        let blockchain = blockchain(
            [mint_account_json()],
            [
                token_account_json(&pubkey(0x44), &pubkey(0x22)),
                Value::Null,
                Value::Null,
                foreign,
            ],
        );

        let auction = auction(vec![
            order(1, pubkey(0x66)),
            order(2, ata),
            order(3, pubkey(0x67)),
            order(4, pubkey(0x68)),
        ]);
        let resolved = auction
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain,
                &BuyTokenAccountCache::default(),
            )
            .await
            .unwrap();

        assert_eq!(uids(&resolved.missing), [2]);
        assert_eq!(uids(&resolved.unreceivable), [3, 4]);
    }

    /// The associated token address derives under the buy mint's token
    /// program: the Token-2022 mint's absent Token-2022 ATA is the
    /// settlement's to create, while the same owner's SPL Token ATA for it is
    /// not.
    #[tokio::test]
    async fn buy_atas_derive_under_the_buy_mints_token_program() {
        let (owner, mint) = (pubkey(0x22), pubkey(0x44));
        let blockchain = blockchain(
            [account_json(&token_2022_mint(&[], |_| ()))],
            [Value::Null, Value::Null],
        );

        let auction = auction(vec![
            order(
                1,
                associated_token_address(&owner, &mint, TokenProgram::Token2022),
            ),
            order(
                2,
                associated_token_address(&owner, &mint, TokenProgram::SplToken),
            ),
        ]);
        let resolved = auction
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain,
                &BuyTokenAccountCache::default(),
            )
            .await
            .unwrap();

        assert_eq!(uids(&resolved.missing), [1]);
        assert_eq!(uids(&resolved.unreceivable), [2]);
    }

    /// The mint lookup answers absent for the buy mint, so no settlement can
    /// pay the order out, whatever its buy token account holds.
    #[tokio::test]
    async fn an_order_with_an_invalid_buy_mint_is_unreceivable() {
        let blockchain = blockchain(
            [Value::Null],
            [token_account_json(&pubkey(0x44), &pubkey(0x22))],
        );
        let auction = auction(vec![order(1, pubkey(0x66))]);

        let resolved = auction
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain,
                &BuyTokenAccountCache::default(),
            )
            .await
            .unwrap();

        assert!(resolved.missing.is_empty());
        assert_eq!(uids(&resolved.unreceivable), [1]);
    }

    /// A native SOL buy pays out to a wallet, so it has no mint or token
    /// account to classify: both lookups answer only for the token buy, an
    /// SPL Token mint and an account absent at the owner's associated token
    /// address.
    #[tokio::test]
    async fn a_native_sol_buy_has_no_buy_token_account_to_resolve() {
        let blockchain = blockchain([mint_account_json()], [Value::Null]);
        let native_buy = Order {
            buy_token: ENCODED_NATIVE_SOL_TRANSFER,
            ..order(1, pubkey(0x68))
        };
        let token_buy = order(
            2,
            associated_token_address(&pubkey(0x22), &pubkey(0x44), TokenProgram::SplToken),
        );

        let resolved = auction(vec![native_buy, token_buy])
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain,
                &BuyTokenAccountCache::default(),
            )
            .await
            .unwrap();

        assert_eq!(uids(&resolved.missing), [2]);
        assert!(resolved.unreceivable.is_empty());
    }

    #[tokio::test]
    async fn fails_when_the_lookup_fails() {
        let blockchain = blockchain([mint_account_json()], [json!("not an account list")]);
        let auction = auction(vec![order(1, pubkey(0x66))]);

        auction
            .resolve_buy_token_accounts(
                Id::new(1).unwrap(),
                &blockchain,
                &BuyTokenAccountCache::default(),
            )
            .await
            .expect_err("a failed lookup fails the resolution");
    }

    /// The lookup answers in order: the first order's PDA exists, the
    /// second's does not.
    #[tokio::test]
    async fn drops_the_creations_whose_order_pda_exists() {
        let existing = json!({
            "lamports": 1u64,
            "data": ["", "base64"],
            "owner": pubkey(0xaa).to_string(),
            "executable": false,
            "rentEpoch": 0u64,
            "space": 0u64,
        });
        let mocks = MocksMap::from_iter([(
            RpcRequest::GetMultipleAccounts,
            multiple_accounts_json([existing, Value::Null]),
        )]);
        let mut auction = auction(vec![
            order(1, pubkey(0x66)),
            Order {
                order_pda: pubkey(0x78),
                ..order(2, pubkey(0x66))
            },
        ]);
        auction.creations = HashMap::from([
            (OrderUid([1; 32]), VersionedTransaction::default()),
            (OrderUid([2; 32]), VersionedTransaction::default()),
        ]);

        auction
            .drop_landed_creations(&blockchain_with_mocks_map(mocks))
            .await;

        assert_eq!(
            auction.creations.keys().collect::<Vec<_>>(),
            [&OrderUid([2; 32])]
        );
    }

    #[tokio::test]
    async fn a_failed_lookup_keeps_every_creation() {
        let mocks = MocksMap::from_iter([(
            RpcRequest::GetMultipleAccounts,
            json!("not an account list"),
        )]);
        let mut auction = auction(vec![order(1, pubkey(0x66))]);
        auction.creations = HashMap::from([(OrderUid([1; 32]), VersionedTransaction::default())]);

        auction
            .drop_landed_creations(&blockchain_with_mocks_map(mocks))
            .await;

        assert_eq!(auction.creations.len(), 1);
    }

    #[test]
    fn an_untouched_order_remains_whole() {
        let order = order(1, pubkey(0x66));
        assert_eq!(
            order.remaining(),
            Remaining {
                sell: 1_000,
                buy: 2_000
            }
        );
    }

    /// The sell balance caps a partially fillable order: 400 of 1000 sold
    /// leaves 600 to sell, a balance of 300 halves both legs. For a buy
    /// order the balance bounds the sell leg: 400 of 2000 bought leaves
    /// 1600 to buy for 800, a balance of 333 allows 666 to buy for 333. A
    /// balance covering the remainder, an unknown one, or a fill-or-kill
    /// order leave the remainder alone.
    #[test]
    fn available_scales_the_remainder_down_to_the_sell_balance() {
        let sell = Order {
            partially_fillable: true,
            executed: 400,
            sell_balance: Some(300),
            ..order(1, pubkey(0x66))
        };
        assert_eq!(
            sell.remaining(),
            Remaining {
                sell: 600,
                buy: 1_200
            }
        );
        assert_eq!(
            sell.available(),
            Remaining {
                sell: 300,
                buy: 600
            }
        );
        for unscaled in [
            Order {
                sell_balance: Some(600),
                ..sell.clone()
            },
            Order {
                sell_balance: None,
                ..sell.clone()
            },
            Order {
                partially_fillable: false,
                ..sell.clone()
            },
        ] {
            assert_eq!(unscaled.available(), sell.remaining(), "{unscaled:?}");
        }
        assert_eq!(
            Order {
                sell_balance: Some(0),
                ..sell.clone()
            }
            .available(),
            Remaining { sell: 0, buy: 0 }
        );

        let buy = Order {
            side: Side::Buy,
            sell_balance: Some(333),
            ..sell
        };
        assert_eq!(
            buy.remaining(),
            Remaining {
                sell: 800,
                buy: 1_600
            }
        );
        assert_eq!(
            buy.available(),
            Remaining {
                sell: 333,
                buy: 666
            }
        );
    }

    /// 999 of 1000 sold leaves 1 to sell; the 2000 buy limit scales to 2,
    /// and a limit that does not divide evenly rounds up against the fill.
    #[test]
    fn a_partially_filled_sell_scales_the_buy_leg_up() {
        let order = Order {
            executed: 999,
            ..order(1, pubkey(0x66))
        };
        assert_eq!(order.remaining(), Remaining { sell: 1, buy: 2 });
        let order = Order {
            buy_amount: 2_001,
            ..order
        };
        assert_eq!(order.remaining(), Remaining { sell: 1, buy: 3 });
    }

    /// 1999 of 2000 bought leaves 1 to buy; the 1000 sell limit scales to
    /// 0.5 and rounds down against the fill.
    #[test]
    fn a_partially_filled_buy_scales_the_sell_leg_down() {
        let order = Order {
            side: Side::Buy,
            executed: 1_999,
            ..order(1, pubkey(0x66))
        };
        assert_eq!(order.remaining(), Remaining { sell: 0, buy: 1 });
        assert!(order.remaining().has_zero_leg());
        assert!(
            !Order {
                executed: 1_998,
                ..order
            }
            .remaining()
            .has_zero_leg()
        );
    }

    #[test]
    fn remaining_amounts_survive_u64_products() {
        let order = Order {
            sell_amount: u64::MAX,
            buy_amount: u64::MAX,
            executed: 1,
            ..order(1, pubkey(0x66))
        };
        assert_eq!(
            order.remaining(),
            Remaining {
                sell: u64::MAX - 1,
                buy: u64::MAX - 1
            }
        );
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
