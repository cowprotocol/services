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
            persistence::{FastPathOrder, FastPathPromotion, StagedFastPathCompetition, dto},
            solvers::dto::settle,
        },
        settle_call::SettleCall,
    },
    alloy::primitives::{Address, U256},
    bigdecimal::BigDecimal,
    chrono::{DateTime, Utc},
    database::byte_array::ByteArray,
    futures::{StreamExt, channel::mpsc},
    model::order::OrderKind,
    number::conversions::u256_to_big_decimal,
    std::{sync::Arc, time::Instant},
    tracing::{Instrument, instrument},
    winner_selection as winsel,
};

pub struct FastPathHandler {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    drivers: Vec<Arc<infra::Driver>>,
    protocol_fees: Arc<domain::ProtocolFees>,
    surplus_capturing_jit_order_owners: Arc<Vec<Address>>,
    settle_coordinator: Arc<SettleCall>,
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
        settle_coordinator: Arc<SettleCall>,
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

    /// Flushes the pre-startup backlog and then spawns the fast-path
    /// listener: pulls order uids off `receiver` and dispatches each
    /// one on a fresh task so a slow handler run never blocks
    /// subsequent orders.
    ///
    /// Awaits the backlog flush before starting the receiver so a
    /// live notification that lands during startup isn't dropped by
    /// the concurrent bulk update.
    pub async fn spawn(self: Arc<Self>, mut receiver: mpsc::UnboundedReceiver<domain::OrderUid>) {
        self.process_order_backlog().await;
        tokio::spawn(async move {
            while let Some(order_uid) = receiver.next().await {
                tokio::spawn(
                    self.clone()
                        .handle(order_uid)
                        .instrument(tracing::info_span!("fast_path", ?order_uid)),
                );
            }
        });
    }

    /// Fast-path orders sitting with `valid_from IS NULL` when the
    /// autopilot starts up are the backlog left behind by a restart or
    /// downtime — their `new_order` notification is gone and by the
    /// time we see them the intended exclusivity window may have long
    /// elapsed. Trying to run the fast-path against a stale quote
    /// would just settle against moved on-chain prices, so instead
    /// bulk-update all of them to `valid_from = now()` and let the
    /// next regular auction pick them up.
    async fn process_order_backlog(&self) {
        let now = model::time::now_in_epoch_seconds();
        match self.persistence.flush_pending_fast_path_backlog(now).await {
            Ok(0) => {}
            Ok(count) => tracing::info!(count, "flushed fast-path backlog into regular auction"),
            Err(err) => tracing::error!(?err, "failed to flush fast-path backlog on startup"),
        }
    }

    /// Classifies a single fast-path order. See the module docs for the
    /// three cases. Non-fast-path orders (the notifier fires for every
    /// new order) short-circuit before any `valid_from` write happens —
    /// only orders the handler actually owns get touched.
    #[instrument(skip_all)]
    async fn handle(self: Arc<Self>, order_uid: domain::OrderUid) {
        let notified_at = Instant::now();
        // Source of truth: the DB. Regular orders (and fast-path
        // orders the handler already classified) return `None` here
        // and are left completely alone.
        let pending = match self.persistence.pending_fast_path_order(order_uid).await {
            Ok(Some(pending)) => pending,
            Ok(None) => {
                tracing::trace!("not a fast path order");
                return;
            }
            Err(err) => {
                Metrics::settlement_not_initiated("db_lookup");
                tracing::error!(?err, "failed to look up pending fast-path order");
                return;
            }
        };

        let creation_date = pending.model_order.metadata.creation_date;

        let settle_attempt = match self.fast_path_enabled {
            true => match self.try_build_settle_request(pending).await {
                Ok(attempt) => Some(attempt),
                Err(err) => {
                    Metrics::settlement_not_initiated(err.reason());
                    tracing::warn!(?err, "could not finalize fast path settle attempt");
                    None
                }
            },
            false => {
                Metrics::settlement_not_initiated("disabled");
                None
            }
        };

        if let Some(settle_attempt) = settle_attempt {
            tracing::debug!(
                auction_id = settle_attempt.settle_request.auction_id,
                "initiating fast path execution"
            );
            Metrics::notify_to_settle(notified_at.elapsed());
            self.execute_fast_path_settle(settle_attempt, creation_date)
                .await
        } else {
            // fast path handling is disabled or order is not viable for fast
            // path execution. To not ignore this order forever we simply set
            // `valid_from: now()` so that the regular auction build picks it
            // up going forward.
            tracing::debug!("fast path not possible, making order valid immediately");
            let now = model::time::now_in_epoch_seconds();
            if let Err(err) = self.persistence.set_order_valid_from(order_uid, now).await {
                tracing::error!(?err, "failed to fall through to regular auction");
            }
        };
    }

    /// Decides whether an out-of-competition settle can be attempted:
    /// only when the runtime feature is enabled, staged competition
    /// data is present (i.e. the order went through the API quoter),
    /// and the fee-adjusted limit-price check passes. Anything else
    /// returns `None` and the caller drops the order into the next
    /// regular auction by writing `valid_from = now()`.
    #[instrument(skip_all)]
    async fn try_build_settle_request(
        &self,
        pending: FastPathOrder,
    ) -> Result<FastPathSettleAttempt, PreflightError> {
        let staged = pending.staged.ok_or(PreflightError::NoStagedData)?;

        // Volume-only: fast-path settles at the quoted price, so any
        // policy that requires a surplus baseline doesn't apply.
        let volume_fee_policies: Vec<_> = self
            .protocol_fees
            .apply(
                &pending.model_order,
                None,
                &self.surplus_capturing_jit_order_owners,
            )
            .into_iter()
            .filter(|p| matches!(p, domain::fee::Policy::Volume { .. }))
            .collect();

        let volume_fee_factors = volume_fee_policies
            .iter()
            .filter_map(|policy| match policy {
                domain::fee::Policy::Volume { factor } => Some(*factor),
                _ => None,
            });

        let winner = staged.winner();
        shared::fee::check_fast_path_limit_fits(
            pending.model_order.data.kind,
            pending.model_order.data.sell_amount,
            pending.model_order.data.buy_amount,
            winner.quoted_sell,
            winner.quoted_buy,
            volume_fee_factors,
        )
        .map_err(|_| PreflightError::LimitTooTight)?;

        let winner = self
            .drivers
            .iter()
            .find(|driver| driver.submission_address == winner.solver)
            .ok_or(PreflightError::DriverNotConfigured(winner.solver))?;

        let deadline = self.submission_deadline();

        let auction_id = staged.data.auction_id;
        let solution_id = staged.winner().solution_id;
        let solution_uid = staged.winner().solution_uid;
        let final_execution = self
            .compute_and_persist_final_execution(
                pending.model_order,
                staged,
                volume_fee_policies,
                &deadline,
            )
            .await?;

        Ok(FastPathSettleAttempt {
            settle_request: settle::Request {
                auction_id,
                solution_id,
                submission_deadline_latest_block: deadline.block,
                fast_path: Some(settle::FastPath {
                    order: dto::order::from_domain(&final_execution.order),
                    limit_prices: settle::LimitPrices {
                        sell: final_execution.limit_sell,
                        buy: final_execution.limit_buy,
                    },
                }),
            },
            winner: winner.clone(),
            solution_uid,
        })
    }

    /// Promotes the staged competition, claims the exclusivity window,
    /// and hands the `/settle` request to the driver. Errors bubble up
    /// to the caller so the handler can log them uniformly.
    #[instrument(skip_all)]
    async fn execute_fast_path_settle(
        &self,
        attempt: FastPathSettleAttempt,
        creation_date: DateTime<Utc>,
    ) {
        let res = self
            .settle_coordinator
            .settle(
                &attempt.winner,
                attempt.winner.submission_address,
                attempt.solution_uid,
                attempt.settle_request,
            )
            .await;
        Metrics::fast_path_finished(&attempt.winner.name, res.is_ok());
        match res {
            Ok(tx) => {
                Metrics::creation_to_execution(Utc::now() - creation_date);
                tracing::info!(?tx, "settled order");
            }
            Err(err) => tracing::warn!(?err, "failed to settle order"),
        };
    }

    /// Adjusts every staged solution's bid by the volume-fee policies,
    /// promotes the staged quote competition into the permanent
    /// `competition_auctions` / `proposed_solutions` /
    /// `proposed_trade_executions` tables, claims the exclusivity
    /// window on the order via `valid_from`, and returns the
    /// fully-built [`FinalOrderExecution`] the caller hands to the
    /// driver. Grouping the two persistence writes here keeps the
    /// "we're committing to running this fast-path" moment in one place
    /// — a failure anywhere in this function leaves the row `valid_from
    /// IS NULL` so the regular auction can still pick it up cleanly.
    async fn compute_and_persist_final_execution(
        &self,
        order: model::order::Order,
        staged: StagedFastPathCompetition,
        volume_fee_policies: Vec<domain::fee::Policy>,
        deadline: &SubmissionDeadline,
    ) -> Result<FinalOrderExecution, PreflightError> {
        let order_uid: domain::OrderUid = order.metadata.uid.into();
        let order_kind = order.data.kind;
        let signed_sell = order.data.sell_amount;
        let signed_buy = order.data.buy_amount;
        let uid = ByteArray(order_uid.0);
        let sell_token = ByteArray(staged.data.sell_token.0.0);
        let buy_token = ByteArray(staged.data.buy_token.0.0);
        let side = shared::db_order_conversions::order_kind_into(order_kind);

        // AuctionContext for `winsel::Arbitrator::score`. Native prices come
        // straight from the staged competition (captured at quote time).
        // Including the order UID in `fee_policies` — even with an empty vec
        // — keeps `contributes_to_score` true so the single fast-path order
        // is always scored.
        let scoring_ctx = winsel::AuctionContext {
            fee_policies: [(
                winsel::OrderUid(order_uid.0),
                volume_fee_policies
                    .iter()
                    .copied()
                    .map(Into::into)
                    .collect(),
            )]
            .into_iter()
            .collect(),
            native_prices: staged.data.native_prices.clone(),
            surplus_capturing_jit_order_owners: self
                .surplus_capturing_jit_order_owners
                .iter()
                .copied()
                .collect(),
        };
        let winsel_side = match order_kind {
            OrderKind::Sell => winsel::Side::Sell,
            OrderKind::Buy => winsel::Side::Buy,
        };

        let mut winning_adjusted: Option<(U256, U256)> = None;
        let solution_rows: Vec<database::solver_competition_v2::Solution> = staged
            .data
            .solutions
            .iter()
            .map(|solution| {
                let (adjusted_sell, adjusted_buy) = apply_volume_fees(
                    solution.quoted_sell,
                    solution.quoted_buy,
                    order_kind,
                    &volume_fee_policies,
                );
                if solution.is_winner {
                    // keep the adjusted prices of the winner as those are the
                    // exact prices the solver is supposed to settle the trade
                    // at
                    winning_adjusted = Some((adjusted_sell, adjusted_buy));
                }
                let solution_uid = i64::try_from(solution.solution_uid)
                    .map_err(|_| PreflightError::SolutionIndexOverflow(solution.solution_uid))?;
                let limit_sell = u256_to_big_decimal(&solution.quoted_sell);
                let limit_buy = u256_to_big_decimal(&solution.quoted_buy);

                // Score failures shouldn't abort the whole promotion — the
                // trade can still settle at the quoted price. Log and fall
                // back to 0 so the solution row is still persisted and the
                // reference-score comparison treats it as no-value-added.
                let (filtered_out, score) = match winsel::arbitrator::score(
                    &winsel::Solution::new(
                        solution.solution_id,
                        solution.solver,
                        vec![winsel::Order {
                            uid: winsel::OrderUid(order_uid.0),
                            sell_token: staged.data.sell_token,
                            buy_token: staged.data.buy_token,
                            sell_amount: signed_sell,
                            buy_amount: signed_buy,
                            executed_sell: adjusted_sell,
                            executed_buy: adjusted_buy,
                            side: winsel_side,
                        }],
                    ),
                    &scoring_ctx,
                ) {
                    Ok(score) => (false, score),
                    Err(_err) => (true, U256::ZERO),
                };

                Ok(database::solver_competition_v2::Solution {
                    uid: solution_uid,
                    id: BigDecimal::from(solution.solution_id),
                    solver: ByteArray(solution.solver.0.0),
                    is_winner: solution.is_winner,
                    filtered_out,
                    score: u256_to_big_decimal(&score),
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
            .collect::<Result<Vec<_>, PreflightError>>()?;

        let (limit_sell, limit_buy) = winning_adjusted.ok_or(PreflightError::MissingWinner)?;

        let reference_score = compute_reference_score(staged.data.auction_id, &solution_rows)?;

        self.persistence
            .finalize_fast_path(FastPathPromotion {
                quote_id: staged.quote_id,
                auction_id: staged.data.auction_id,
                order_uid,
                block: deadline.computed_at_block,
                deadline: deadline.block,
                valid_from: deadline.timestamp,
                native_prices: staged.data.native_prices.clone(),
                solutions: solution_rows,
                fee_policies: volume_fee_policies.clone(),
                reference_score,
                // TODO: populate penalty caps correctly. For a brief period after the
                // launch there will be no penalties but we already need to store a
                // 0 value for the accounting pipeline to work.
                penalty_cap_native: 0.into(),
            })
            .await
            .map_err(PreflightError::PersistFailed)?;

        let order = boundary::order::to_domain(&order, volume_fee_policies, None, None);
        Ok(FinalOrderExecution {
            order,
            limit_sell,
            limit_buy,
        })
    }

    /// Computes timestamp and number of the last block the order may be
    /// executed in.
    fn submission_deadline(&self) -> SubmissionDeadline {
        // TODO: for the initial version we just use the same submission
        // deadline as the main auction uses which likely extends beyond
        // the valid_from period. It's still not possible for the same
        // order to be part of the fast path and a regular auction at the
        // same time because the SettleCallCoordinator populates tables
        // which feed the filter of the inflight order detection.
        //
        // The final implementation should take the order's valid_from
        // and the expected end of the current auction into account.
        let block_time_ms = self.eth.chain().block_time_in_ms().as_millis() as u64;
        let window_secs = self
            .submission_deadline
            .saturating_mul(block_time_ms)
            .div_ceil(1000);
        let current_block = self.eth.current_block().borrow();
        // Anchor to the current block's on-chain timestamp — the
        // deadline block will arrive at approximately `current +
        // submission_deadline × block_time` seconds after this one.
        // Using wall-clock `now` instead would overestimate mid-block
        // (an order placed 6s into a 12s slot would push `valid_from`
        // out by a whole block's worth).
        SubmissionDeadline {
            computed_at_block: current_block.number,
            timestamp: (current_block.timestamp + window_secs) as u32,
            block: current_block.number + self.submission_deadline,
        }
    }
}

/// Bundle of everything the settle path needs; computed by
/// [`FastPathHandler::try_build_settle_attempt`] once we know the order
/// clears the limit-price check.
struct FastPathSettleAttempt {
    winner: Arc<infra::Driver>,
    solution_uid: usize,
    settle_request: settle::Request,
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

struct SubmissionDeadline {
    /// Block at which this deadline was computed at.
    computed_at_block: u64,
    /// Last block at which the order may be executed in.
    block: u64,
    /// UNIX timestamp at which the target block will be mined
    timestamp: u32,
}

/// Errors that prevent the actual fast path execution.
#[derive(Debug, thiserror::Error)]
enum PreflightError {
    #[error("no staged competition data available")]
    NoStagedData,
    #[error("fast-path limit check failed; falling through to regular auction")]
    LimitTooTight,
    #[error("winning driver {0:?} is currently not configured")]
    DriverNotConfigured(Address),
    #[error("solution index {0} does not fit in i64")]
    SolutionIndexOverflow(usize),
    #[error("staged competition has no solution flagged as winner")]
    MissingWinner,
    #[error("failed to finalize the fast path data in the DB")]
    PersistFailed(#[from] anyhow::Error),
}

impl PreflightError {
    fn reason(&self) -> &'static str {
        match self {
            Self::NoStagedData => "no_staged_data",
            Self::LimitTooTight => "limit_too_tight",
            Self::DriverNotConfigured(_) => "driver_not_configured",
            Self::SolutionIndexOverflow(_) => "solution_index_overflow",
            Self::MissingWinner => "missing_winner",
            Self::PersistFailed(_) => "persist_failed",
        }
    }
}

/// Computes reference score for the winning solver: the *counterfactual*
/// score — what would have been achieved without this solver.
/// This is defined as the second best score or 0 if there was only 1
/// solution.
fn compute_reference_score(
    auction_id: database::auction::AuctionId,
    solution_rows: &[database::solver_competition_v2::Solution],
) -> Result<database::reference_scores::Score, PreflightError> {
    let Some(winner) = solution_rows
        .iter()
        .find(|s| s.is_winner && !s.filtered_out)
    else {
        // for some reason there is no winner - abort execution to keep data
        // consistent
        tracing::error!(?solution_rows, "no winner for reference score computation");
        return Err(PreflightError::MissingWinner);
    };
    let reference_score = solution_rows
        .iter()
        .filter(|s| !s.is_winner && !s.filtered_out)
        .map(|s| &s.score)
        .max()
        .cloned()
        .unwrap_or_else(|| BigDecimal::from(0));
    Ok(database::reference_scores::Score {
        auction_id,
        solver: winner.solver,
        reference_score,
    })
}

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "fast_path")]
struct Metrics {
    /// Tracks the outcome of fast-path settle requests.
    #[metric(labels("driver", "result"))]
    executions: prometheus::IntCounterVec,
    /// Counts errors that prevented the autopilot from
    /// initiating a fast-path settle request.
    #[metric(labels("reason"))]
    errors: prometheus::IntCounterVec,
    /// Seconds between the order's `creation_date` and observing
    /// the settlement onchain.
    #[metric(buckets(0.5, 1, 1.5, 2, 2.5, 3, 4, 5, 6, 7, 8, 9, 10, 12, 24))]
    total_duration: prometheus::Histogram,
    /// Seconds between the autopilot receiving the fast-path
    /// notification for an order and firing the driver `/settle` call.
    #[metric(buckets(0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2, 5))]
    processing_latency: prometheus::Histogram,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }

    fn fast_path_finished(solver: &str, success: bool) {
        let result = if success { "success" } else { "failure" };
        Self::get()
            .executions
            .with_label_values(&[solver, result])
            .inc();
    }

    fn settlement_not_initiated(reason: &str) {
        Self::get().errors.with_label_values(&[reason]).inc();
    }

    fn notify_to_settle(elapsed: std::time::Duration) {
        Self::get()
            .processing_latency
            .observe(elapsed.as_secs_f64());
    }

    fn creation_to_execution(elapsed: chrono::Duration) {
        // Clamp to zero to guard against clock skew between the
        // orderbook (which stamped `creation_date`) and the autopilot.
        let secs = (elapsed.num_milliseconds() as f64 / 1000.0).max(0.0);
        Self::get().total_duration.observe(secs);
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
