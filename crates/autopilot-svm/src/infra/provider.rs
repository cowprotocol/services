//! Auction provider backed by the indexer-written tables.

use {
    crate::{
        domain::{auction::Order, cycle::SolanaCycle},
        infra::{db, inflight::InFlightOrders, prices::NativePrices},
        run_loop::AuctionProvider,
    },
    async_trait::async_trait,
    chain_types::solana::Pubkey as ChainPubkey,
    cow_solana_rpc::SolanaRPC,
    solana_sdk::{account::Account, program_pack::Pack, pubkey::Pubkey},
    spl_token_interface::state::{Account as TokenAccount, AccountState},
    sqlx::PgPool,
    std::time::{SystemTime, UNIX_EPOCH},
};

/// Cuts auctions from the open orders the indexer persisted.
pub struct DbAuctionProvider {
    pool: PgPool,
    rpc: SolanaRPC,
    /// Slots the indexer may lag behind the tip before cuts are skipped.
    max_indexer_lag: u64,
    /// Orders with a settlement in flight, excluded from cuts until their
    /// submission deadline passes.
    inflight: InFlightOrders,
    prices: NativePrices,
}

impl DbAuctionProvider {
    pub fn new(
        pool: PgPool,
        rpc: SolanaRPC,
        max_indexer_lag: u64,
        inflight: InFlightOrders,
        prices: NativePrices,
    ) -> Self {
        Self {
            pool,
            rpc,
            max_indexer_lag,
            inflight,
            prices,
        }
    }

    /// Drop orders whose buy token account cannot receive the settlement
    /// payout: their settlement would revert at `FinalizeSettle`. Only orders
    /// already created on chain are checked, a pending sponsored order
    /// creates its own accounts at settlement time. When the account lookup
    /// fails every order passes, a doomed order then costs one failed
    /// settlement instead of the whole cut.
    async fn receivable_orders(&self, orders: Vec<Order>) -> Vec<Order> {
        let candidates = orders
            .iter()
            .filter(|order| order.created_on_chain)
            .map(|order| Pubkey::new_from_array(order.buy_token_account.0));
        let accounts = match self.rpc.multiple_accounts(candidates).await {
            Ok(accounts) => accounts,
            Err(err) => {
                tracing::warn!(?err, "buy account lookup failed, keeping all orders");
                return orders;
            }
        };
        orders
            .into_iter()
            .filter(|order| {
                if !order.created_on_chain {
                    return true;
                }
                let account = Pubkey::new_from_array(order.buy_token_account.0);
                let receivable = accounts
                    .get(&account)
                    .is_some_and(|found| receivable_token_account(found, order.buy_token.0));
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
            .collect()
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
        let mut orders = db::cut(&self.pool, now, block_height)
            .await
            .map_err(|err| tracing::warn!(?err, "failed to cut the auction"))
            .ok()?;
        // An order with a settlement in flight stays out until the
        // settlement cannot land any more: a second winner could
        // double-settle it.
        let held = self.inflight.held_at(*tip);
        let before = orders.len();
        orders.retain(|order| !held.contains(&order.uid));
        let held_out = before - orders.len();
        if held_out > 0 {
            metrics()
                .held_out_orders
                .inc_by(u64::try_from(held_out).unwrap_or(u64::MAX));
            tracing::debug!(held_out, "orders held out with settlements in flight");
        }
        let orders = self.receivable_orders(orders).await;
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
        let tip_slot = i64::try_from(*tip).unwrap_or(i64::MAX);
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

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::domain::auction::OrderKind,
        chain_types::solana::{AppData, IntentHash, Pubkey as ChainPubkey},
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
            InFlightOrders::default(),
            NativePrices::seeded([]),
        )
    }

    /// The lookup answers for the three created orders in candidate order:
    /// initialized with the buy mint, initialized with a wrong mint, absent.
    /// The pending sponsored order is exempt from the check.
    #[tokio::test]
    async fn drops_created_orders_with_unreceivable_buy_accounts() {
        let response = serde_json::json!({
            "context": {"slot": 1u64, "apiVersion": "2.0.0"},
            "value": [
                crate::tests::token_account_json([0x44; 32]),
                crate::tests::token_account_json([0x99; 32]),
                null,
            ],
        });
        let provider = provider(Mocks::from([(RpcRequest::GetMultipleAccounts, response)]));
        let orders = vec![
            order([0x01; 32], true),
            order([0x02; 32], false),
            order([0x03; 32], true),
            order([0x04; 32], true),
        ];
        let kept: Vec<u8> = provider
            .receivable_orders(orders)
            .await
            .iter()
            .map(|order| order.buy_token_account.0[0])
            .collect();
        assert_eq!(kept, [0x01, 0x02]);
    }

    /// A failed lookup (here a malformed response) keeps every order.
    #[tokio::test]
    async fn keeps_all_orders_when_the_lookup_fails() {
        let provider = provider(Mocks::from([(
            RpcRequest::GetMultipleAccounts,
            serde_json::json!("not an account list"),
        )]));
        let orders = vec![order([0x01; 32], true)];
        assert_eq!(provider.receivable_orders(orders).await.len(), 1);
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
