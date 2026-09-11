//! Handles a fast-path order the moment it lands: computes the applicable
//! fee policies, promotes the staged quote competition into the permanent
//! competition tables, and hands the resulting `/settle` request to a
//! [`SettleCallCoordinator`].
//!
//! [`FastPathHandler::spawn`] wires the handler up to an
//! `mpsc::UnboundedReceiver<OrderUid>` fed by the DB order notifier
//! (`infra::order_notify::fast_path::FastPathNotifier`), so the handler
//! runs independently of the regular auction run loop.

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
    submission_deadline: u64,
}

impl FastPathHandler {
    pub fn new(
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        protocol_fees: Arc<domain::ProtocolFees>,
        surplus_capturing_jit_order_owners: Arc<Vec<Address>>,
        settle_coordinator: Arc<SettleCallCoordinator>,
        submission_deadline: u64,
    ) -> Arc<Self> {
        Arc::new(Self {
            eth,
            persistence,
            drivers,
            protocol_fees,
            surplus_capturing_jit_order_owners,
            settle_coordinator,
            submission_deadline,
        })
    }

    /// Spawns the fast-path listener: pulls order uids off `receiver`, looks
    /// each one up in `quote_competitions`, and — if a row exists for a yet
    /// to be finalized fast-path order — dispatches to [`Self::handle`] on a
    /// fresh task so a slow handler run never blocks subsequent orders.
    pub fn spawn(self: Arc<Self>, mut receiver: mpsc::UnboundedReceiver<domain::OrderUid>) {
        tokio::spawn(async move {
            while let Some(order_uid) = receiver.next().await {
                let this = self.clone();
                tokio::spawn(
                    async move {
                        match this.persistence.fast_path_order(order_uid).await {
                            // Not a fast-path order — nothing to do.
                            Ok(None) => {}
                            Err(err) => {
                                tracing::error!(?err, "failed to look up fast path order")
                            }
                            Ok(Some(order)) => this.handle(order).await,
                        };
                    }
                    .instrument(tracing::info_span!("fast_path", ?order_uid)),
                );
            }
        });
    }

    /// Handles a fast-path order. Picks a final submission deadline in the
    /// exclusivity period and instructs the winning solver to settle
    /// directly and outside the regular auction.
    #[instrument(skip_all)]
    pub async fn handle(&self, fast_path_data: FastPathOrder) {
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
            .compute_and_persist_final_execution(&fast_path_data, current_block, deadline)
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

    /// Computes the fee policies the order would receive in a regular
    /// auction, adjusts every staged solution's bid to reflect them,
    /// promotes the staged quote competition into the permanent
    /// `competition_auctions` / `proposed_solutions` /
    /// `proposed_trade_executions` tables, and returns the fully-built
    /// [`FinalOrderExecution`] the caller hands to the driver.
    ///
    /// In the fast path only volume-based fees make sense: no surplus over
    /// the quote is expected. All bid adjustment and the assembly of the
    /// resulting `domain::Order` happens here so the infra side is a
    /// straight-line transaction of DB inserts and the caller has a single
    /// value to forward.
    async fn compute_and_persist_final_execution(
        &self,
        fast_path_data: &FastPathOrder,
        block: u64,
        deadline: u64,
    ) -> anyhow::Result<FinalOrderExecution> {
        let order_uid: domain::OrderUid = fast_path_data.model_order.metadata.uid.into();
        let volume_fee_policies: Vec<_> = self
            .protocol_fees
            .apply(
                &fast_path_data.model_order,
                None,
                &self.surplus_capturing_jit_order_owners,
            )
            .into_iter()
            // the fast path execution is strictly tied to promised amounts
            // during quoting, only volume fees make sense under those
            // conditions
            .filter(|p| matches!(p, domain::fee::Policy::Volume { .. }))
            .collect();

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
                    &volume_fee_policies,
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
                fee_policies: volume_fee_policies.clone(),
            })
            .await?;

        let order = boundary::order::to_domain(
            &fast_path_data.model_order,
            volume_fee_policies,
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
