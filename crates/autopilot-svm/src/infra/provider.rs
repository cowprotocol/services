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
    solana_sdk::{account::Account, program_pack::Pack, pubkey::Pubkey, rent::Rent},
    spl_token_interface::{
        native_mint,
        state::{Account as TokenAccount, AccountState},
    },
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

    /// Drop orders whose buy token account cannot receive the settlement
    /// payout: their settlement would revert at `FinalizeSettle`. A native SOL
    /// buy pays a wallet instead, which must be missing or owned by the System
    /// Program. A pending sponsored token buy skips the check, its creation
    /// transaction creates the buy token account. When the account lookup
    /// fails every order passes, a doomed order then costs one failed
    /// settlement instead of the whole cut. Returns the kept orders and the
    /// uids of the dropped ones.
    async fn receivable_orders(&self, orders: Vec<Order>) -> (Vec<Order>, Vec<IntentHash>) {
        let checked = |order: &Order| order.created_on_chain || order.buys_native_sol();
        let candidates = orders
            .iter()
            .filter(|order| checked(order))
            .map(|order| Pubkey::new_from_array(order.buy_token_account.0));
        let accounts = match self.rpc.multiple_accounts(candidates).await {
            Ok(accounts) => accounts,
            Err(err) => {
                tracing::warn!(?err, "buy account lookup failed, keeping all orders");
                return (orders, Vec::new());
            }
        };
        let mut dropped = Vec::new();
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
                    // TODO: flip on once the buy token account rent is priced.
                    // None => account == associated_token_address(order),
                };
                if !receivable {
                    dropped.push(order.uid);
                }
                receivable
            })
            .collect();
        (orders, dropped)
    }

    /// Record the orders a cut leaves out for `reason`: the per-reason gauge,
    /// a debug line with their uids and a `filtered` order event. An order
    /// repeats this on every cut while the reason holds, so the line stays at
    /// debug. The gauge is set on every cut, so an empty reason reads zero.
    fn track_filtered_orders(&self, reason: OrderFilterReason, uids: Vec<IntentHash>) {
        metrics()
            .filtered_orders
            .with_label_values(&[reason.as_str()])
            .set(i64::try_from(uids.len()).unwrap_or(i64::MAX));
        if uids.is_empty() {
            return;
        }
        let orders: Vec<String> = uids.iter().map(ToString::to_string).collect();
        tracing::debug!(
            reason = reason.as_str(),
            count = orders.len(),
            ?orders,
            "filtered orders"
        );
        order_events::store_detached(self.pool.clone(), uids, OrderEventLabel::Filtered);
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
        self.track_filtered_orders(
            OrderFilterReason::InFlight,
            held_out.into_iter().map(|order| order.uid).collect(),
        );
        let (orders, unpayable) = payable_orders(orders);
        self.track_filtered_orders(OrderFilterReason::UnpayableNativeBuy, unpayable);
        let (orders, unreceivable) = self.receivable_orders(orders).await;
        self.track_filtered_orders(OrderFilterReason::UnreceivableBuyTokenAccount, unreceivable);
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

/// Why an auction cut leaves an order out.
#[derive(Clone, Copy)]
enum OrderFilterReason {
    /// A settlement from an earlier auction can still land.
    InFlight,
    /// The settlement cannot pay out the native SOL buy.
    UnpayableNativeBuy,
    /// The buy token account cannot receive the payout.
    UnreceivableBuyTokenAccount,
}

impl OrderFilterReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::InFlight => "in_flight",
            Self::UnpayableNativeBuy => "unpayable_native_buy",
            Self::UnreceivableBuyTokenAccount => "unreceivable_buy_token_account",
        }
    }
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "auction_provider")]
struct Metrics {
    /// Orders the last auction cut left out, by reason.
    #[metric(labels("reason"))]
    filtered_orders: prometheus::IntGaugeVec,
    /// Auction cuts skipped because the indexer lags beyond the watermark.
    /// The loop keeps spinning and stays live through a skip, so this
    /// counter is the alerting signal for a stalled indexer.
    lag_skipped_cuts: prometheus::IntCounter,
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
/// classic SPL token program, the one account a settlement can create for
/// the payout when it does not exist yet.
/// TODO(token-2022): a token-2022 mint derives a different address, so its
/// missing account is dropped here, like the driver cannot settle it yet.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the receivable check that uses it is held until solvers price the ATA rent"
    )
)]
fn associated_token_address(order: &Order) -> Pubkey {
    spl_associated_token_account_interface::address::get_associated_token_address_with_program_id(
        &Pubkey::new_from_array(order.owner.0),
        &Pubkey::new_from_array(order.buy_token.0),
        &spl_token_interface::ID,
    )
}

/// An initialized, unfrozen account of the classic SPL token program holding
/// the order's buy mint: anything else reverts the payout at settlement.
/// TODO(token-2022): accounts of the token-2022 program are dropped here,
/// like the driver cannot settle them yet.
fn receivable_token_account(account: &Account, buy_mint: [u8; 32]) -> bool {
    account.owner == spl_token_interface::ID
        && TokenAccount::unpack(&account.data).is_ok_and(|account| {
            account.state == AccountState::Initialized && account.mint.to_bytes() == buy_mint
        })
}

/// Drop native SOL buys the settlement cannot pay out. A payout under the
/// rent-exempt minimum of an empty wallet reverts the whole settlement, and a
/// partial fill can land under it. A wSOL sell reaches solvers as wSOL for
/// wSOL. Returns the kept orders and the uids of the dropped ones.
fn payable_orders(orders: Vec<Order>) -> (Vec<Order>, Vec<IntentHash>) {
    // TODO: use the cluster's rent, refreshed periodically. The SDK default is
    // above it since SIMD-0437, so this floor also drops small payouts that
    // would settle.
    let min_payout = Rent::default().minimum_balance(0);
    let (payable, unpayable): (Vec<_>, Vec<_>) = orders.into_iter().partition(|order| {
        !order.buys_native_sol()
            || (order.buy_amount >= min_payout
                && !order.partially_fillable
                && order.sell_token.0 != native_mint::ID.to_bytes())
    });
    (
        payable,
        unpayable.into_iter().map(|order| order.uid).collect(),
    )
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
    /// address. An absent account is dropped either way while the settlement
    /// creating it is held. The pending sponsored order is exempt from the
    /// check.
    #[tokio::test]
    async fn drops_created_orders_with_unreceivable_buy_accounts() {
        let response = serde_json::json!({
            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
            "value": [
                crate::tests::token_account_json([0x44; 32]),
                crate::tests::token_account_json([0x99; 32]),
                null,
                null,
            ],
        });
        let provider = provider(Mocks::from([(RpcRequest::GetMultipleAccounts, response)]));
        let ata = associated_token_address(&order([0; 32], true)).to_bytes();
        let orders = vec![
            order([0x01; 32], true),
            order([0x02; 32], false),
            order([0x03; 32], true),
            order([0x04; 32], true),
            order(ata, true),
        ];
        let (kept, dropped) = provider.receivable_orders(orders).await;
        let kept: Vec<[u8; 32]> = kept.iter().map(|order| order.buy_token_account.0).collect();
        assert_eq!(kept, [[0x01; 32], [0x02; 32]]);
        assert_eq!(
            dropped,
            [
                IntentHash([0x03; 32]),
                IntentHash([0x04; 32]),
                IntentHash(ata)
            ]
        );
    }

    /// A native SOL buy pays its wallet directly: a missing or system-owned
    /// wallet receives it, an account of another program does not. A pending
    /// sponsored native buy gets the same check.
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
        let (kept, dropped) = payable_orders(orders.clone());
        assert_eq!(kept, [orders[0].clone(), orders[4].clone()]);
        assert_eq!(dropped.len(), 3);
    }

    /// The gauge reads the last cut's count, zero once the reason clears.
    #[tokio::test]
    async fn tracks_filtered_orders_per_reason() {
        let provider = provider(Mocks::default());
        let gauge = || {
            metrics()
                .filtered_orders
                .with_label_values(&[OrderFilterReason::InFlight.as_str()])
                .get()
        };
        provider
            .track_filtered_orders(OrderFilterReason::InFlight, vec![IntentHash([0x01; 32]); 2]);
        assert_eq!(gauge(), 2);
        provider.track_filtered_orders(OrderFilterReason::InFlight, Vec::new());
        assert_eq!(gauge(), 0);
    }

    /// A failed lookup (here a malformed response) keeps every order.
    #[tokio::test]
    async fn keeps_all_orders_when_the_lookup_fails() {
        let provider = provider(Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!("not an account list"),
        )]));
        let orders = vec![order([0x01; 32], true)];
        let (kept, dropped) = provider.receivable_orders(orders).await;
        assert_eq!((kept.len(), dropped.len()), (1, 0));
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
