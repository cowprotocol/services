//! Auction provider backed by the indexer-written tables.

use {
    crate::{
        domain::{auction::Order, cycle::SolanaCycle},
        infra::{db, order_events, prices::NativePrices},
        run_loop::AuctionProvider,
    },
    async_trait::async_trait,
    chain_types::solana::{IntentHash, Pubkey as ChainPubkey},
    cow_solana_rpc::SolanaRPC,
    database::solana::OrderEventLabel,
    solana_sdk::{account::Account, pubkey::Pubkey, rent::Rent},
    spl_token_2022_interface::{
        extension::{
            BaseStateWithExtensions,
            StateWithExtensions,
            default_account_state::DefaultAccountState,
            memo_transfer::memo_required,
            pausable::PausableConfig,
            transfer_fee::TransferFeeConfig,
            transfer_hook::TransferHook,
        },
        state::{Account as TokenAccount, AccountState, Mint},
    },
    spl_token_interface::native_mint,
    sqlx::PgPool,
    std::{
        collections::HashSet,
        time::{SystemTime, UNIX_EPOCH},
    },
};

/// Cuts auctions from the open orders the indexer persisted.
pub struct DbAuctionProvider {
    pool: PgPool,
    rpc: SolanaRPC,
    /// Slots the indexer may lag behind the tip before cuts are skipped.
    max_indexer_lag: u64,
    prices: NativePrices,
}

impl DbAuctionProvider {
    pub fn new(pool: PgPool, rpc: SolanaRPC, max_indexer_lag: u64, prices: NativePrices) -> Self {
        Self {
            pool,
            rpc,
            max_indexer_lag,
            prices,
        }
    }

    /// Drop orders whose settlement would revert. The program must be able to
    /// move the sell and buy mints, and the buy token account must receive the
    /// payout at `FinalizeSettle`. A native SOL buy pays a wallet instead,
    /// which must be missing or owned by the System Program. A pending
    /// sponsored token buy skips the account check, its creation transaction
    /// creates the buy token account. The mints and the accounts share one
    /// lookup. When it fails every order passes, a doomed order then costs one
    /// failed settlement instead of the whole cut. Returns the kept orders and
    /// the uids of those dropped for a mint.
    async fn receivable_orders(&self, orders: Vec<Order>) -> (Vec<Order>, Vec<IntentHash>) {
        let checked = |order: &Order| order.created_on_chain || order.buys_native_sol();
        let buy_accounts = orders
            .iter()
            .filter(|order| checked(order))
            .map(|order| Pubkey::new_from_array(order.buy_token_account.0));
        let mints = orders.iter().flat_map(token_mints);
        let accounts = match self.rpc.multiple_accounts(buy_accounts.chain(mints)).await {
            Ok(accounts) => accounts,
            Err(err) => {
                tracing::warn!(?err, "order account lookup failed, keeping all orders");
                return (orders, Vec::new());
            }
        };
        let (orders, unsettleable): (Vec<_>, Vec<_>) = orders.into_iter().partition(|order| {
            let unsettleable = token_mints(order)
                .find_map(|mint| Some((mint, unsettleable_mint(accounts.get(&mint))?)));
            if let Some((mint, reason)) = unsettleable {
                metrics().unsettleable_orders.inc();
                tracing::debug!(
                    order = %order.uid,
                    %mint,
                    ?reason,
                    "excluding order, the settlement program cannot move its mint"
                );
            }
            unsettleable.is_none()
        });
        let orders = orders
            .into_iter()
            .filter(|order| {
                if !checked(order) {
                    return true;
                }
                let account = Pubkey::new_from_array(order.buy_token_account.0);
                let receivable = match accounts.get(&account) {
                    found if order.buys_native_sol() => found.is_none_or(receivable_wallet),
                    Some(found) => receivable_token_account(found, order.buy_token.0),
                    None => false,
                    // TODO: flip on once the buy token account rent is priced,
                    // with the program that owns the buy mint account.
                    // None => account == associated_token_address(order, &program),
                };
                if !receivable {
                    // A doomed order repeats this on every cut until it
                    // expires: the counter is the alerting signal, the log
                    // line stays at debug.
                    metrics().unreceivable_orders.inc();
                    tracing::debug!(
                        order = %order.uid,
                        %account,
                        "excluding order, its buy token account cannot receive the payout"
                    );
                }
                receivable
            })
            .collect();
        (
            orders,
            unsettleable.into_iter().map(|order| order.uid).collect(),
        )
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after the unix epoch")
        .as_secs()
        .try_into()
        .expect("unix seconds fit i64")
}

#[async_trait]
impl AuctionProvider<SolanaCycle> for DbAuctionProvider {
    /// The order data is push-fed by the indexer, there is no cache to
    /// refresh.
    async fn sync_to_tip(&self, _tip: &u64) -> anyhow::Result<()> {
        Ok(())
    }

    async fn cut_auction(&self, tip: &u64) -> Option<crate::domain::auction::Auction> {
        // An auction cut from a lagging indexer replays stale orders, so a
        // lag beyond the watermark skips the cycle. A failed read cuts
        // anyway: the same pool fails the cut itself one query later.
        match db::last_indexed_slot(&self.pool).await {
            Ok(indexed) if indexer_lags(*tip, indexed, self.max_indexer_lag) => {
                tracing::warn!(
                    tip,
                    ?indexed,
                    "the indexer lags beyond the watermark, skipping the cut"
                );
                metrics().lag_skipped_cuts.inc();
                return None;
            }
            Ok(_) => {}
            Err(err) => tracing::warn!(?err, "indexer slot read failed"),
        }
        let now = now_unix();
        // A pending sponsored order dies with its creation blockhash, so the
        // cut drops the dead ones. A failed height fetch keeps them all: they
        // then fall out at the countersign instead of the cut.
        let block_height = match self.rpc.block_height().await {
            Ok(height) => Some(i64::try_from(u64::from(height)).unwrap_or(i64::MAX)),
            Err(err) => {
                tracing::warn!(?err, "block height lookup failed, keeping pending orders");
                None
            }
        };
        let orders = db::cut(&self.pool, now, block_height)
            .await
            .map_err(|err| tracing::warn!(?err, "failed to cut the auction"))
            .ok()?;
        // An order with a settlement in flight stays out until the
        // settlement cannot land any more: a second winner could
        // double-settle it. A failed read skips the cut rather than cutting
        // without the hold.
        let tip_slot = i64::try_from(*tip).unwrap_or(i64::MAX);
        let held: HashSet<IntentHash> = match db::in_flight_orders(&self.pool, tip_slot).await {
            Ok(uids) => uids.into_iter().map(|uid| IntentHash(uid.0)).collect(),
            Err(err) => {
                tracing::warn!(?err, "in-flight order lookup failed, skipping the cut");
                return None;
            }
        };
        let (orders, held_out): (Vec<_>, Vec<_>) = orders
            .into_iter()
            .partition(|order| !held.contains(&order.uid));
        if !held_out.is_empty() {
            metrics()
                .held_out_orders
                .inc_by(u64::try_from(held_out.len()).unwrap_or(u64::MAX));
            tracing::debug!(
                held_out = held_out.len(),
                "orders held out with settlements in flight"
            );
            order_events::store_detached(
                self.pool.clone(),
                held_out.into_iter().map(|order| order.uid).collect(),
                OrderEventLabel::Filtered,
            );
        }
        let (orders, unsettleable) = self.receivable_orders(payable_orders(orders)).await;
        if !unsettleable.is_empty() {
            order_events::store_detached(
                self.pool.clone(),
                unsettleable,
                OrderEventLabel::Filtered,
            );
        }
        if orders.is_empty() {
            return None;
        }
        // A cut without prices would rank solutions on incomparable scores,
        // so a failed lookup skips the cycle instead.
        let tokens = orders
            .iter()
            .flat_map(|order| [order.sell_token, order.buy_token])
            .map(|token| Pubkey::new_from_array(token.0))
            .collect();
        let prices = match self.prices.prices(tokens).await {
            Ok(prices) => prices,
            Err(err) => {
                tracing::warn!(?err, "native price lookup failed, skipping the cut");
                return None;
            }
        };
        let mut auction = crate::domain::auction::Auction {
            id: 0,
            orders,
            native_prices: prices
                .into_iter()
                .map(|(token, price)| (ChainPubkey(token.to_bytes()), price))
                .collect(),
        };
        // The id must be durable before anything references it: windows and
        // the competition snapshot key on it, so a failed write skips the
        // cycle.
        let snapshot = auction_snapshot(*tip, &auction);
        auction.id = match db::replace_current_auction(&self.pool, tip_slot, &snapshot).await {
            Ok(id) => id,
            Err(err) => {
                tracing::warn!(?err, "failed to store the auction, skipping the cut");
                return None;
            }
        };
        Some(auction)
    }
}

/// The stored auction body: the solver-facing content without the deadline,
/// which is only known at dispatch.
fn auction_snapshot(tip: u64, auction: &crate::domain::auction::Auction) -> serde_json::Value {
    serde_json::json!({
        "tipSlot": tip,
        "orders": auction
            .orders
            .iter()
            .map(|order| order.uid.to_string())
            .collect::<Vec<_>>(),
        "nativePrices": auction
            .native_prices
            .iter()
            .map(|(token, price)| {
                (
                    Pubkey::new_from_array(token.0).to_string(),
                    price.to_string(),
                )
            })
            .collect::<std::collections::HashMap<_, _>>(),
    })
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "auction_provider")]
struct Metrics {
    /// Orders excluded from auction cuts because their buy token account
    /// cannot receive the payout.
    unreceivable_orders: prometheus::IntCounter,
    /// Orders excluded from auction cuts because the settlement program cannot
    /// move their sell or buy mint.
    unsettleable_orders: prometheus::IntCounter,
    /// Native SOL buys excluded from auction cuts because the settlement
    /// cannot pay them out.
    unpayable_native_buys: prometheus::IntCounter,
    /// Auction cuts skipped because the indexer lags beyond the watermark.
    /// The loop keeps spinning and stays live through a skip, so this
    /// counter is the alerting signal for a stalled indexer.
    lag_skipped_cuts: prometheus::IntCounter,
    /// Orders excluded from auction cuts while their settlement is in
    /// flight.
    held_out_orders: prometheus::IntCounter,
}

fn metrics() -> &'static Metrics {
    Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
}

/// Whether the indexer's processed slot trails the tip beyond the allowed
/// lag. A missing slot means the indexer never wrote, which counts as
/// maximal lag.
fn indexer_lags(tip: u64, indexed: Option<i64>, max_lag: u64) -> bool {
    let Some(indexed) = indexed.and_then(|slot| u64::try_from(slot).ok()) else {
        return true;
    };
    tip.saturating_sub(indexed) > max_lag
}

/// The order owner's associated token account for the buy mint under the
/// mint's token `program`, the one account a settlement can create for the
/// payout when it does not exist yet.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the receivable check that uses it is held until solvers price the ATA rent"
    )
)]
fn associated_token_address(order: &Order, program: &Pubkey) -> Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &Pubkey::new_from_array(order.owner.0),
        &Pubkey::new_from_array(order.buy_token.0),
        program,
    )
}

/// An initialized, unfrozen token account holding the order's buy mint. It
/// must not require memos on incoming transfers, since the payout carries
/// none. Anything else reverts the payout at settlement.
fn receivable_token_account(account: &Account, buy_mint: [u8; 32]) -> bool {
    token_program_owned(account)
        && StateWithExtensions::<TokenAccount>::unpack(&account.data).is_ok_and(|state| {
            state.base.state == AccountState::Initialized
                && state.base.mint.to_bytes() == buy_mint
                && !memo_required(&state)
        })
}

/// The token mints an order moves: the sell mint, and the buy mint unless the
/// order buys native SOL.
fn token_mints(order: &Order) -> impl Iterator<Item = Pubkey> {
    let buy = (!order.buys_native_sol()).then_some(order.buy_token);
    std::iter::once(order.sell_token)
        .chain(buy)
        .map(|mint| Pubkey::new_from_array(mint.0))
}

/// Why the settlement program cannot move a mint's tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsettleableMint {
    /// The account is missing or not a mint of either token program.
    NotAMint,
    /// Token-2022 rejects the program's plain `Transfer` for this mint, even
    /// at a zero fee.
    TransferFee,
    /// Token-2022 rejects the program's plain `Transfer` for this mint, even
    /// without a hook program.
    TransferHook,
    /// New token accounts start frozen, so the buffer and payer account the
    /// settlement creates cannot receive the tokens.
    FrozenByDefault,
    /// Every transfer fails while the mint is paused.
    Paused,
}

/// Why the settlement program cannot move the tokens of the mint at
/// `account`, `None` when it can.
///
/// TODO(BE-320): a permanent delegate mint passes, although its issuer can
/// move the buffer's balance of the token, retained fees included.
fn unsettleable_mint(account: Option<&Account>) -> Option<UnsettleableMint> {
    let Some(mint) = account
        .filter(|account| token_program_owned(account))
        .and_then(|account| StateWithExtensions::<Mint>::unpack(&account.data).ok())
    else {
        return Some(UnsettleableMint::NotAMint);
    };
    if mint.get_extension::<TransferFeeConfig>().is_ok() {
        Some(UnsettleableMint::TransferFee)
    } else if mint.get_extension::<TransferHook>().is_ok() {
        Some(UnsettleableMint::TransferHook)
    } else if mint
        .get_extension::<DefaultAccountState>()
        .is_ok_and(|default| default.state == AccountState::Frozen as u8)
    {
        Some(UnsettleableMint::FrozenByDefault)
    } else if mint
        .get_extension::<PausableConfig>()
        .is_ok_and(|pausable| bool::from(pausable.paused))
    {
        Some(UnsettleableMint::Paused)
    } else {
        None
    }
}

/// Whether one of the two token programs, classic SPL or Token-2022, owns
/// `account`.
fn token_program_owned(account: &Account) -> bool {
    account.owner == spl_token_interface::ID || account.owner == spl_token_2022_interface::ID
}

/// Drop native SOL buys the settlement cannot pay out. A payout under the
/// rent-exempt minimum of an empty wallet reverts the whole settlement, and a
/// partial fill can land under it. A wSOL sell reaches solvers as wSOL for
/// wSOL.
fn payable_orders(mut orders: Vec<Order>) -> Vec<Order> {
    // TODO: use the cluster's rent, refreshed periodically. The SDK default is
    // above it since SIMD-0437, so this floor also drops small payouts that
    // would settle.
    let min_payout = Rent::default().minimum_balance(0);
    orders.retain(|order| {
        let payable = !order.buys_native_sol()
            || (order.buy_amount >= min_payout
                && !order.partially_fillable
                && order.sell_token.0 != native_mint::ID.to_bytes());
        if !payable {
            metrics().unpayable_native_buys.inc();
            tracing::debug!(
                order = %order.uid,
                "excluding native SOL buy, the settlement cannot pay it out"
            );
        }
        payable
    });
    orders
}

/// Whether `account` is a System Program wallet. Lamports paid to a program
/// or a sysvar revert the settlement, a program-owned account strands them.
fn receivable_wallet(account: &Account) -> bool {
    account.owner == solana_system_interface::program::ID
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::auction::OrderKind,
        chain_types::solana::{AppData, IntentHash, NATIVE_SOL, Pubkey as ChainPubkey},
        cow_solana_rpc::{Mocks, RpcRequest},
        solana_sdk::program_pack::Pack,
        spl_token_2022_interface::extension::{
            BaseStateWithExtensionsMut,
            ExtensionType,
            StateWithExtensionsMut,
            immutable_owner::ImmutableOwner,
            memo_transfer::MemoTransfer,
            mint_close_authority::MintCloseAuthority,
            permanent_delegate::PermanentDelegate,
        },
    };

    fn order(buy_token_account: [u8; 32], created_on_chain: bool) -> Order {
        Order {
            uid: IntentHash(buy_token_account),
            owner: ChainPubkey([0x22; 32]),
            sell_token: ChainPubkey([0x33; 32]),
            buy_token: ChainPubkey([0x44; 32]),
            sell_token_account: ChainPubkey([0x55; 32]),
            buy_token_account: ChainPubkey(buy_token_account),
            sell_amount: 1_000,
            buy_amount: 2_000,
            valid_to: 42,
            kind: OrderKind::Sell,
            partially_fillable: false,
            order_pda: ChainPubkey([0x77; 32]),
            app_data: AppData([0; 32]),
            created_on_chain,
        }
    }

    fn provider(mocks: Mocks) -> DbAuctionProvider {
        DbAuctionProvider::new(
            sqlx::PgPool::connect_lazy("postgresql://").unwrap(),
            SolanaRPC::new_mock_with_mocks(mocks),
            150,
            NativePrices::seeded([]),
        )
    }

    /// The lookup answers for the four created orders in candidate order:
    /// initialized with the buy mint, initialized with a wrong mint, absent
    /// at an arbitrary address, absent at the owner's associated token
    /// address. The sell and buy mints follow. An absent account is dropped
    /// either way while the settlement creating it is held. The pending
    /// sponsored order is exempt from the check.
    #[tokio::test]
    async fn drops_created_orders_with_unreceivable_buy_accounts() {
        let response = serde_json::json!({
            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
            "value": [
                crate::tests::token_account_json([0x44; 32]),
                crate::tests::token_account_json([0x99; 32]),
                null,
                null,
                crate::tests::mint_account_json(6),
                crate::tests::mint_account_json(6),
            ],
        });
        let provider = provider(Mocks::from([(RpcRequest::GetMultipleAccounts, response)]));
        let ata =
            associated_token_address(&order([0; 32], true), &spl_token_interface::ID).to_bytes();
        let orders = vec![
            order([0x01; 32], true),
            order([0x02; 32], false),
            order([0x03; 32], true),
            order([0x04; 32], true),
            order(ata, true),
        ];
        let kept: Vec<[u8; 32]> = provider
            .receivable_orders(orders)
            .await
            .0
            .iter()
            .map(|order| order.buy_token_account.0)
            .collect();
        assert_eq!(kept, [[0x01; 32], [0x02; 32]]);
    }

    /// A native SOL buy pays its wallet directly: a missing or system-owned
    /// wallet receives it, an account of another program does not. A pending
    /// sponsored native buy gets the same check. Only the sell mint follows
    /// the wallets in the lookup.
    #[tokio::test]
    async fn native_buys_pay_system_wallets() {
        let system_wallet = serde_json::json!({
            "lamports": 1_000_000u64,
            "data": ["", "base64"],
            "owner": "11111111111111111111111111111111",
            "executable": false,
            "rentEpoch": 0u64,
            "space": 0u64,
        });
        let response = serde_json::json!({
            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
            "value": [
                null,
                system_wallet,
                crate::tests::token_account_json([0x44; 32]),
                crate::tests::token_account_json([0x44; 32]),
                crate::tests::mint_account_json(6),
            ],
        });
        let provider = provider(Mocks::from([(RpcRequest::GetMultipleAccounts, response)]));
        let native = |wallet, created_on_chain| Order {
            buy_token: NATIVE_SOL,
            ..order(wallet, created_on_chain)
        };
        let orders = vec![
            native([0x01; 32], true),
            native([0x02; 32], true),
            native([0x03; 32], true),
            native([0x04; 32], false),
        ];
        let kept: Vec<[u8; 32]> = provider
            .receivable_orders(orders)
            .await
            .0
            .iter()
            .map(|order| order.buy_token_account.0)
            .collect();
        assert_eq!(kept, [[0x01; 32], [0x02; 32]]);
    }

    /// Native SOL buys stay out under the rent-exempt minimum of an empty
    /// account, when partially fillable, and when they sell wSOL. Token buys
    /// pass whatever their amount.
    #[test]
    fn drops_native_buys_the_settlement_cannot_pay() {
        let native = |buy_amount| Order {
            buy_token: NATIVE_SOL,
            buy_amount,
            ..order([0x01; 32], true)
        };
        let orders = vec![
            native(890_880),
            native(890_879),
            Order {
                partially_fillable: true,
                ..native(1_000_000_000)
            },
            Order {
                sell_token: ChainPubkey(native_mint::ID.to_bytes()),
                ..native(1_000_000_000)
            },
            Order {
                buy_amount: 1,
                ..order([0x02; 32], true)
            },
        ];
        assert_eq!(
            payable_orders(orders.clone()),
            [orders[0].clone(), orders[4].clone()]
        );
    }

    /// A failed lookup (here a malformed response) keeps every order.
    #[tokio::test]
    async fn keeps_all_orders_when_the_lookup_fails() {
        let provider = provider(Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!("not an account list"),
        )]));
        let orders = vec![order([0x01; 32], true)];
        assert_eq!(provider.receivable_orders(orders).await.0.len(), 1);
    }

    /// Orders on a mint the program cannot move stay out, on either side of
    /// the trade, and their uids come back for the `Filtered` event. A mint
    /// missing from the lookup counts as one the program cannot move.
    #[tokio::test]
    async fn drops_orders_on_mints_the_program_cannot_move() {
        let fee_mint = crate::tests::token_2022_mint(&[ExtensionType::TransferFeeConfig], |mint| {
            mint.init_extension::<TransferFeeConfig>(true).unwrap();
        });
        // The pending sponsored orders skip the buy account check, so the
        // lookup reads only the mints, in first-seen order: 0x33, 0x44, 0x88,
        // 0x99.
        let response = serde_json::json!({
            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
            "value": [
                crate::tests::mint_account_json(6),
                crate::tests::mint_account_json(6),
                crate::tests::account_json(&fee_mint),
                null,
            ],
        });
        let provider = provider(Mocks::from([(RpcRequest::GetMultipleAccounts, response)]));
        let orders = vec![
            order([0x01; 32], false),
            Order {
                sell_token: ChainPubkey([0x88; 32]),
                ..order([0x02; 32], false)
            },
            Order {
                buy_token: ChainPubkey([0x88; 32]),
                ..order([0x03; 32], false)
            },
            Order {
                sell_token: ChainPubkey([0x99; 32]),
                ..order([0x04; 32], false)
            },
        ];

        let (kept, unsettleable) = provider.receivable_orders(orders).await;
        assert_eq!(
            kept.iter().map(|order| order.uid).collect::<Vec<_>>(),
            [IntentHash([0x01; 32])]
        );
        assert_eq!(
            unsettleable,
            [
                IntentHash([0x02; 32]),
                IntentHash([0x03; 32]),
                IntentHash([0x04; 32])
            ]
        );
    }

    /// An initialized classic SPL Token mint.
    fn classic_mint() -> Account {
        let mut data = vec![0; Mint::LEN];
        Mint {
            is_initialized: true,
            decimals: 6,
            ..Mint::default()
        }
        .pack_into_slice(&mut data);
        Account {
            owner: spl_token_interface::ID,
            data,
            ..Account::default()
        }
    }

    /// The program moves classic mints and Token-2022 mints whose extensions
    /// leave a plain transfer alone, a permanent delegate included. Transfer
    /// fees and hooks fail it, and so do frozen-by-default and paused mints.
    #[test]
    fn classifies_mints_by_their_extensions() {
        let with = |extension, init: fn(&mut StateWithExtensionsMut<Mint>)| {
            unsettleable_mint(Some(&crate::tests::token_2022_mint(&[extension], init)))
        };
        assert_eq!(unsettleable_mint(Some(&classic_mint())), None);
        assert_eq!(
            with(ExtensionType::MintCloseAuthority, |mint| {
                mint.init_extension::<MintCloseAuthority>(true).unwrap();
            }),
            None
        );
        assert_eq!(
            with(ExtensionType::PermanentDelegate, |mint| {
                mint.init_extension::<PermanentDelegate>(true)
                    .unwrap()
                    .delegate = Some(Pubkey::new_unique()).try_into().unwrap();
            }),
            None
        );
        assert_eq!(
            with(ExtensionType::TransferFeeConfig, |mint| {
                mint.init_extension::<TransferFeeConfig>(true).unwrap();
            }),
            Some(UnsettleableMint::TransferFee)
        );
        assert_eq!(
            with(ExtensionType::TransferHook, |mint| {
                mint.init_extension::<TransferHook>(true).unwrap();
            }),
            Some(UnsettleableMint::TransferHook)
        );
        assert_eq!(
            with(ExtensionType::DefaultAccountState, |mint| {
                mint.init_extension::<DefaultAccountState>(true)
                    .unwrap()
                    .state = AccountState::Frozen as u8;
            }),
            Some(UnsettleableMint::FrozenByDefault)
        );
        assert_eq!(
            with(ExtensionType::DefaultAccountState, |mint| {
                mint.init_extension::<DefaultAccountState>(true)
                    .unwrap()
                    .state = AccountState::Initialized as u8;
            }),
            None
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap().paused = true.into();
            }),
            Some(UnsettleableMint::Paused)
        );
        assert_eq!(
            with(ExtensionType::Pausable, |mint| {
                mint.init_extension::<PausableConfig>(true).unwrap();
            }),
            None
        );
    }

    /// A missing account, a token account and a mint layout under another
    /// program are no mints the program can move.
    #[test]
    fn only_token_program_mints_are_settleable() {
        let foreign = Account {
            owner: solana_system_interface::program::ID,
            ..classic_mint()
        };
        assert_eq!(unsettleable_mint(None), Some(UnsettleableMint::NotAMint));
        assert_eq!(
            unsettleable_mint(Some(&token_2022_account([0x44; 32], false))),
            Some(UnsettleableMint::NotAMint)
        );
        assert_eq!(
            unsettleable_mint(Some(&foreign)),
            Some(UnsettleableMint::NotAMint)
        );
    }

    /// An initialized Token-2022 account of `mint` with the `MemoTransfer`
    /// extension, requiring memos on incoming transfers or not.
    fn token_2022_account(mint: [u8; 32], require_memos: bool) -> Account {
        let len = ExtensionType::try_calculate_account_len::<TokenAccount>(&[
            ExtensionType::ImmutableOwner,
            ExtensionType::MemoTransfer,
        ])
        .unwrap();
        let mut data = vec![0; len];
        let mut account =
            StateWithExtensionsMut::<TokenAccount>::unpack_uninitialized(&mut data).unwrap();
        account.init_extension::<ImmutableOwner>(true).unwrap();
        account
            .init_extension::<MemoTransfer>(true)
            .unwrap()
            .require_incoming_transfer_memos = require_memos.into();
        account.base = TokenAccount {
            mint: Pubkey::new_from_array(mint),
            state: AccountState::Initialized,
            ..TokenAccount::default()
        };
        account.pack_base();
        account.init_account_type().unwrap();
        Account {
            owner: spl_token_2022_interface::ID,
            data,
            ..Account::default()
        }
    }

    /// A Token-2022 buy account receives the payout unless it requires memos
    /// on incoming transfers, which the payout does not carry.
    #[test]
    fn token_2022_buy_accounts_receive_without_a_memo_requirement() {
        assert!(receivable_token_account(
            &token_2022_account([0x44; 32], false),
            [0x44; 32]
        ));
        assert!(!receivable_token_account(
            &token_2022_account([0x44; 32], true),
            [0x44; 32]
        ));
        assert!(!receivable_token_account(
            &token_2022_account([0x99; 32], false),
            [0x44; 32]
        ));
    }

    /// The watermark trips past the allowed lag, on a never-written indexer,
    /// and on a nonsensical negative slot.
    #[test]
    fn detects_indexer_lag() {
        assert!(!indexer_lags(100, Some(100), 10));
        assert!(!indexer_lags(100, Some(90), 10));
        assert!(indexer_lags(100, Some(89), 10));
        assert!(!indexer_lags(89, Some(100), 10));
        assert!(indexer_lags(100, None, 10));
        assert!(indexer_lags(100, Some(-1), 10));
    }
}
