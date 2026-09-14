//! Handles a fast-path order the moment it lands: decides whether to
//! attempt an out-of-competition settlement via the driver's `/settle`
//! or to fall through to the next regular auction, and populates the
//! order's `valid_from` accordingly.
//!
//! The autopilot owns `valid_from` on every fast-path order. The
//! orderbook / on-chain indexer just record `orders.fast_path = true`
//! at placement; the row stays `NOT (fast_path AND valid_from IS NULL)`
//! excluded from the solvable-orders cache until this handler writes
//! the timestamp.
//!
//! Three cases:
//! - `fast_path_enabled = false` at runtime: write `valid_from = now()` so the
//!   order flows straight into the next regular auction.
//! - No staged quote competition (e.g. an ethflow fast-path order that never
//!   went through the quoter) or the fee-adjusted limit check fails: same,
//!   `valid_from = now()`. The caller still opted into fast-path treatment, but
//!   the autopilot can't honour it.
//! - Otherwise: initiate the fast-path settlement and write `valid_from = now +
//!   submission_deadline × chain block time`, so the order is barred from a
//!   regular auction for exactly as long as the settle attempt runs — no
//!   separate wall-clock knob to drift out of sync with the block deadline.
//!
//! [`FastPathHandler::spawn`] wires the handler up to an
//! `mpsc::UnboundedReceiver<OrderUid>` fed by the DB order notifier
//! (`infra::order_notify::fast_path::FastPathNotifier`), so the handler
//! runs independently of the regular auction run loop. On start
//! [`FastPathHandler::process_order_backlog`] re-drives the handler for
//! orders whose notification landed while the process was down.

use {
    crate::{
        boundary,
        domain,
        infra::{
            self,
            persistence::{FastPathOrder, FastPathPromotion, dto},
            solvers::dto::settle,
        },
        settle_call_coordinator::SettleCallCoordinator,
    },
    alloy::primitives::{Address, U256},
    anyhow::Context,
    bigdecimal::BigDecimal,
    configs::fee_factor::FeeFactor,
    database::byte_array::ByteArray,
    futures::{StreamExt, channel::mpsc},
    model::order::OrderKind,
    number::conversions::u256_to_big_decimal,
    std::sync::Arc,
    tracing::{Instrument, instrument},
};

pub struct FastPathHandler {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    drivers: Vec<Arc<infra::Driver>>,
    protocol_fees: Arc<domain::ProtocolFees>,
    surplus_capturing_jit_order_owners: Arc<Vec<Address>>,
    settle_coordinator: Arc<SettleCallCoordinator>,
    /// Block-count deadline for the fast-path settle attempt. Doubles
    /// as the exclusivity window in wall clock: `valid_from` is set to
    /// `now + submission_deadline × chain.block_time`, so the order
    /// isn't picked up by the regular auction until the settle attempt
    /// has definitely elapsed. Reusing the same knob for both keeps
    /// them from drifting out of sync.
    submission_deadline: u64,
    /// Runtime toggle: when `false`, the handler skips the driver
    /// `/settle` call entirely and just writes `valid_from = now()` so
    /// the order flows into the next regular auction.
    fast_path_enabled: bool,
}

impl FastPathHandler {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        protocol_fees: Arc<domain::ProtocolFees>,
        surplus_capturing_jit_order_owners: Arc<Vec<Address>>,
        settle_coordinator: Arc<SettleCallCoordinator>,
        submission_deadline: u64,
        fast_path_enabled: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            eth,
            persistence,
            drivers,
            protocol_fees,
            surplus_capturing_jit_order_owners,
            settle_coordinator,
            submission_deadline,
            fast_path_enabled,
        })
    }

    /// Wall-clock length of the fast-path exclusivity window: the number
    /// of blocks the settle attempt has (`submission_deadline`) scaled
    /// by the network's block time. Rounded up so the regular auction
    /// only picks the order up once the settle deadline has definitely
    /// elapsed.
    fn exclusivity_secs(&self) -> u64 {
        let block_time_ms = self.eth.chain().block_time_in_ms().as_millis() as u64;
        let window_ms = self.submission_deadline.saturating_mul(block_time_ms);
        window_ms.div_ceil(1_000)
    }

    /// Spawns the fast-path listener: pulls order uids off `receiver`
    /// and dispatches each one on a fresh task so a slow handler run
    /// never blocks subsequent orders.
    pub fn spawn(self: Arc<Self>, mut receiver: mpsc::UnboundedReceiver<domain::OrderUid>) {
        tokio::spawn(self.clone().process_order_backlog());
        tokio::spawn(async move {
            while let Some(order_uid) = receiver.next().await {
                self.clone().spawn_order_handler(order_uid);
            }
        });
    }

    /// The autopilot is responsible for populating the `valid_from`
    /// column in the orders table. Additionally fast path orders that
    /// don't have a `valid_from` yet are not allowed to be part of
    /// the auction. That means whenever there was a downtime or also
    /// just during a regular restart the autopilot needs to process
    /// the backlog of unfinalized orders which is what this function
    /// is doing.
    async fn process_order_backlog(self: Arc<Self>) {
        let uids = match self.persistence.pending_fast_path_order_uids().await {
            Ok(uids) => uids,
            Err(err) => {
                tracing::error!(?err, "failed to fetch pending fast-path orders on startup");
                return;
            }
        };
        if !uids.is_empty() {
            tracing::info!(count = uids.len(), "processing fast path order backlog");
        }
        for uid in uids {
            self.clone().spawn_order_handler(uid);
        }
    }

    fn spawn_order_handler(self: Arc<Self>, order_uid: domain::OrderUid) {
        tokio::spawn(
            async move {
                self.handle(order_uid).await;
            }
            .instrument(tracing::info_span!("fast_path", ?order_uid)),
        );
    }

    /// Classifies a single fast-path order. See the module docs for the
    /// three cases.
    #[instrument(skip_all)]
    async fn handle(&self, order_uid: domain::OrderUid) {
        let now = model::time::now_in_epoch_seconds() as i64;

        // Try to line up a fast-path settle. If any prerequisite is
        // missing we still need to write `valid_from` so the order
        // can enter the next regular auction.
        let settle_attempt = self.prepare_fast_path_settle(order_uid).await;

        let valid_from = if settle_attempt.is_some() {
            now + self.exclusivity_secs().cast_signed()
        } else {
            now
        };

        if let Err(err) = self
            .persistence
            .set_order_valid_from(order_uid, valid_from)
            .await
        {
            tracing::error!(?err, "failed to set valid_from on fast-path order");
            return;
        }

        if let Some(attempt) = settle_attempt {
            self.execute_fast_path_settle(attempt).await;
        }
    }

    /// Loads the staged quote competition and runs the same fee-adjusted
    /// limit-price check the orderbook enforces at placement. Returns
    /// `None` when the order isn't eligible for a fast-path settle —
    /// feature disabled, no staged competition (ethflow), or the check
    /// fails. Callers use `Some` as the signal to actually attempt the
    /// settle and to extend `valid_from`.
    async fn prepare_fast_path_settle(
        &self,
        order_uid: domain::OrderUid,
    ) -> Option<FastPathSettleAttempt> {
        // Feature disabled: skip the DB lookup entirely.
        if !self.fast_path_enabled {
            return None;
        }

        let fast_path_data = match self.persistence.fast_path_order(order_uid).await {
            Ok(Some(data)) => data,
            Ok(None) => {
                // Either not a pending fast-path order at all or no
                // staged competition (ethflow). Nothing to settle out
                // of band.
                return None;
            }
            Err(err) => {
                tracing::error!(?err, "failed to look up staged fast-path competition");
                return None;
            }
        };

        // Volume-only: fast-path settles at the quoted price, so any
        // policy that requires a surplus baseline doesn't apply.
        let volume_fee_policies: Vec<_> = self
            .protocol_fees
            .apply(
                &fast_path_data.model_order,
                None,
                &self.surplus_capturing_jit_order_owners,
            )
            .into_iter()
            .filter(|p| matches!(p, domain::fee::Policy::Volume { .. }))
            .collect();

        let factors = volume_fee_policies
            .iter()
            .filter_map(|policy| match policy {
                domain::fee::Policy::Volume { factor } => Some(*factor),
                _ => None,
            })
            .collect::<Vec<_>>();

        let winner = fast_path_data.winner();
        if shared::fee::check_fast_path_limit_fits(
            fast_path_data.model_order.data.kind,
            fast_path_data.model_order.data.sell_amount,
            fast_path_data.model_order.data.buy_amount,
            winner.quoted_sell,
            winner.quoted_buy,
            factors.iter().copied(),
        )
        .is_err()
        {
            tracing::info!(
                ?order_uid,
                "fast-path limit check failed; falling through to regular auction"
            );
            Metrics::fast_path_limit_too_tight();
            return None;
        }

        Some(FastPathSettleAttempt {
            fast_path_data,
            volume_fee_policies,
            _factors: factors,
        })
    }

    /// Promotes the staged competition, hands the `/settle` request to
    /// the driver, and records the outcome. Runs after `valid_from`
    /// has already been extended past the exclusivity window.
    async fn execute_fast_path_settle(&self, attempt: FastPathSettleAttempt) {
        let FastPathSettleAttempt {
            fast_path_data,
            volume_fee_policies,
            _factors: _,
        } = attempt;

        let winning_solver = fast_path_data.winner().solver;
        let Some(winner) = self
            .drivers
            .iter()
            .find(|driver| driver.submission_address == winning_solver)
        else {
            tracing::error!(
                solver = ?winning_solver,
                "winning driver is currently not configured"
            );
            return;
        };

        // TODO: for the initial version we just use the same submission
        // deadline as the main auction uses which likely extends beyond
        // the valid_from period. It's still not possible for the same
        // order to be part of the fast path and a regular auction at the
        // same time because the SettleCallCoordinator populates tables
        // which feed the filter of the inflight order detection.
        //
        // The final implementation should take the order's valid_from
        // and the expected end of the current auction into account.
        let current_block = self.eth.current_block().borrow().number;
        let deadline = current_block + self.submission_deadline;

        let final_execution = match self
            .compute_and_persist_final_execution(
                &fast_path_data,
                &volume_fee_policies,
                current_block,
                deadline,
            )
            .await
        {
            Ok(execution) => execution,
            Err(err) => {
                tracing::error!(?err, "failed to record fast-path fee policies");
                return;
            }
        };

        let request = settle::Request {
            auction_id: fast_path_data.staged.auction_id,
            solution_id: fast_path_data.winner().solution_id,
            submission_deadline_latest_block: deadline,
            fast_path: Some(settle::FastPath {
                order: dto::order::from_domain(&final_execution.order),
                limit_prices: settle::LimitPrices {
                    sell: final_execution.limit_sell,
                    buy: final_execution.limit_buy,
                },
            }),
        };

        let res = self
            .settle_coordinator
            .settle(
                winner,
                winner.submission_address,
                fast_path_data.winner().solution_uid,
                request,
            )
            .await;
        Metrics::fast_path_finished(&winner.name, res.is_ok());
        match res {
            Ok(tx) => tracing::info!(?tx, "settled order"),
            Err(err) => tracing::debug!(?err, "failed to settle order"),
        };
    }

    /// Adjusts every staged solution's bid by the volume-fee policies,
    /// promotes the staged quote competition into the permanent
    /// `competition_auctions` / `proposed_solutions` /
    /// `proposed_trade_executions` tables, and returns the
    /// fully-built [`FinalOrderExecution`] the caller hands to the
    /// driver.
    async fn compute_and_persist_final_execution(
        &self,
        fast_path_data: &FastPathOrder,
        volume_fee_policies: &[domain::fee::Policy],
        block: u64,
        deadline: u64,
    ) -> anyhow::Result<FinalOrderExecution> {
        let order_uid: domain::OrderUid = fast_path_data.model_order.metadata.uid.into();
        let order_kind = fast_path_data.model_order.data.kind;
        let uid = ByteArray(order_uid.0);
        let sell_token = ByteArray(fast_path_data.staged.sell_token.0.0);
        let buy_token = ByteArray(fast_path_data.staged.buy_token.0.0);
        let side = shared::db_order_conversions::order_kind_into(order_kind);

        let mut winning_adjusted: Option<(U256, U256)> = None;
        let solution_rows: Vec<database::solver_competition_v2::Solution> = fast_path_data
            .staged
            .solutions
            .iter()
            .map(|solution| {
                let (adjusted_sell, adjusted_buy) = apply_volume_fees(
                    solution.quoted_sell,
                    solution.quoted_buy,
                    order_kind,
                    volume_fee_policies,
                );
                if solution.is_winner {
                    // keep the adjusted prices of the winner as those are the
                    // exact prices the solver is supposed to settle the trade
                    // at
                    winning_adjusted = Some((adjusted_sell, adjusted_buy));
                }
                let solution_uid = i64::try_from(solution.solution_uid)
                    .context("solution index does not fit in i64")?;
                let limit_sell = u256_to_big_decimal(&solution.quoted_sell);
                let limit_buy = u256_to_big_decimal(&solution.quoted_buy);
                Ok(database::solver_competition_v2::Solution {
                    uid: solution_uid,
                    id: BigDecimal::from(solution.solution_id),
                    solver: ByteArray(solution.solver.0.0),
                    is_winner: solution.is_winner,
                    // TODO: populate in a way that is consistent with the usual
                    // winner selection logic
                    filtered_out: false,
                    // TODO: populate in a way that is consisent with the usual
                    // winner selection logic
                    score: BigDecimal::from(0),
                    orders: vec![database::solver_competition_v2::Order {
                        uid,
                        sell_token,
                        buy_token,
                        limit_sell: limit_sell.clone(),
                        limit_buy: limit_buy.clone(),
                        executed_sell: u256_to_big_decimal(&adjusted_sell),
                        executed_buy: u256_to_big_decimal(&adjusted_buy),
                        side,
                    }],
                    // Natural single-trade UCP encoding: sell/buy prices are
                    // just the quoted amounts of the other side.
                    price_tokens: vec![sell_token, buy_token],
                    price_values: vec![limit_buy, limit_sell],
                })
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let (limit_sell, limit_buy) =
            winning_adjusted.expect("winner is present in staged competition");

        self.persistence
            .finalize_fast_path(FastPathPromotion {
                quote_id: fast_path_data.quote_id,
                auction_id: fast_path_data.staged.auction_id,
                order_uid,
                block,
                deadline,
                native_prices: fast_path_data.staged.native_prices.clone(),
                solutions: solution_rows,
                fee_policies: volume_fee_policies.to_vec(),
            })
            .await?;

        let order = boundary::order::to_domain(
            &fast_path_data.model_order,
            volume_fee_policies.to_vec(),
            None,
            None,
        );
        Ok(FinalOrderExecution {
            order,
            limit_sell,
            limit_buy,
        })
    }
}

/// Bundle of everything the settle path needs; computed by
/// [`FastPathHandler::prepare_fast_path_settle`] once we know the order
/// clears the limit-price check.
struct FastPathSettleAttempt {
    fast_path_data: FastPathOrder,
    volume_fee_policies: Vec<domain::fee::Policy>,
    /// Kept for symmetry — the factors are already baked into
    /// `volume_fee_policies`; the field is here so the limit-check math
    /// happens exactly once per handler run.
    _factors: Vec<FeeFactor>,
}

/// Output of the fee-policy computation and bid-adjustment step of the
/// fast-path handler.
struct FinalOrderExecution {
    /// The domain order the fast-path handler forwards to the driver,
    /// already annotated with the applicable fee policies and quote.
    order: domain::Order,
    /// Quoted sell amount after all Volume-type policies are applied.
    limit_sell: U256,
    /// Quoted buy amount after all Volume-type policies are applied.
    limit_buy: U256,
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "runloop")]
struct Metrics {
    /// Tracks the outcome of fast-path settlements.
    #[metric(labels("driver", "result"))]
    fast_path_executions: prometheus::IntCounterVec,
    /// Incremented when the fast-path limit-price check would have
    /// rejected the order at settle time, so we fell through to the
    /// regular auction instead of attempting the fast-path.
    fast_path_limit_too_tight: prometheus::IntCounter,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    fn fast_path_finished(solver: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        Self::get()
            .fast_path_executions
            .with_label_values(&[solver, result])
            .inc();
    }

    fn fast_path_limit_too_tight() {
        Self::get().fast_path_limit_too_tight.inc();
    }
}

/// Applies every `Volume`-type policy in `policies` to `(sell, buy)` in
/// order, compounding each fee on the amount produced by the previous step.
/// Non-volume policies are ignored — callers can pass the raw policy list
/// without pre-filtering. Delegates the per-factor arithmetic to
/// [`shared::fee::apply_volume_fee`] so the sensitive high-precision math
/// lives in exactly one place.
fn apply_volume_fees(
    sell: U256,
    buy: U256,
    kind: OrderKind,
    policies: &[domain::fee::Policy],
) -> (U256, U256) {
    policies
        .iter()
        .filter_map(|policy| match policy {
            domain::fee::Policy::Volume { factor } => Some(*factor),
            _ => None,
        })
        .fold((sell, buy), |(sell, buy), factor| {
            shared::fee::apply_volume_fee(sell, buy, kind, factor)
        })
}

#[cfg(test)]
mod tests {
    // The per-factor arithmetic is covered by `shared::fee` tests; here we
    // only verify the policy filtering / compounding this wrapper adds.
    use super::*;

    fn factor(v: f64) -> configs::fee_factor::FeeFactor {
        configs::fee_factor::FeeFactor::try_from(v).unwrap()
    }

    #[test]
    fn apply_volume_fees_compounds_in_order() {
        let expected = shared::fee::apply_volume_fee(
            U256::from(1_000u64),
            U256::from(990u64),
            OrderKind::Sell,
            factor(0.02),
        );
        let actual = apply_volume_fees(
            U256::from(1_000u64),
            U256::from(1_000u64),
            OrderKind::Sell,
            &[
                domain::fee::Policy::Volume {
                    factor: factor(0.01),
                },
                domain::fee::Policy::Volume {
                    factor: factor(0.02),
                },
            ],
        );
        assert_eq!(expected, actual);
    }

    #[test]
    fn apply_volume_fees_ignores_non_volume_policies() {
        // Non-volume policies get silently skipped so callers can pass the
        // unfiltered list.
        let surplus = domain::fee::Policy::Surplus {
            factor: factor(0.1),
            max_volume_factor: factor(0.5),
        };
        let (sell, buy) = apply_volume_fees(
            U256::from(1_000u64),
            U256::from(1_000u64),
            OrderKind::Sell,
            &[
                surplus,
                domain::fee::Policy::Volume {
                    factor: factor(0.01),
                },
                surplus,
            ],
        );
        // Only the single 1% volume fee took effect.
        assert_eq!(sell, U256::from(1_000u64));
        assert_eq!(buy, U256::from(990u64));
    }
}
