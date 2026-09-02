//! Handles a fast-path order the moment it lands: computes the applicable
//! fee policies, adjusts the recorded bid amounts, and hands the resulting
//! `/settle` request to the shared [`SettleCallCoordinator`].
//!
//! This module owns everything specific to fast-path handling that used to
//! live inline in [`crate::run_loop::RunLoop`].

use {
    super::settle_call_coordinator::SettleCallCoordinator,
    crate::{
        boundary,
        domain,
        infra::{self, persistence::FastPathOrder, solvers::dto::settle},
    },
    alloy::primitives::{Address, U256},
    std::sync::Arc,
    tracing::instrument,
};

pub struct FastPathHandler {
    eth: infra::Ethereum,
    persistence: infra::Persistence,
    drivers: Vec<Arc<infra::Driver>>,
    protocol_fees: Arc<domain::ProtocolFees>,
    surplus_capturing_jit_order_owners: Arc<Vec<Address>>,
    settle_coordinator: Arc<SettleCallCoordinator>,
}

impl FastPathHandler {
    pub fn new(
        eth: infra::Ethereum,
        persistence: infra::Persistence,
        drivers: Vec<Arc<infra::Driver>>,
        protocol_fees: Arc<domain::ProtocolFees>,
        surplus_capturing_jit_order_owners: Arc<Vec<Address>>,
        settle_coordinator: Arc<SettleCallCoordinator>,
    ) -> Arc<Self> {
        Arc::new(Self {
            eth,
            persistence,
            drivers,
            protocol_fees,
            surplus_capturing_jit_order_owners,
            settle_coordinator,
        })
    }

    /// Handles a fast-path order. Picks a final submission deadline in the
    /// exclusivity period and instructs the winning solver to settle
    /// directly and outside the regular auction.
    #[instrument(skip_all)]
    pub async fn handle(&self, fast_path_data: FastPathOrder) {
        let Some(winner) = self
            .drivers
            .iter()
            .find(|driver| driver.submission_address == fast_path_data.solver)
        else {
            tracing::error!(
                solver = ?fast_path_data.solver,
                "winning driver is currently not configured"
            );
            return;
        };

        let AppliedFees {
            policies,
            quote,
            limit_sell,
            limit_buy,
        } = match self.compute_and_persist_fees(&fast_path_data).await {
            Ok(fees) => fees,
            Err(err) => {
                tracing::error!(?err, "failed to record fast-path fee policies");
                return;
            }
        };

        let domain_order =
            boundary::order::to_domain(&fast_path_data.model_order, policies, Some(quote), None);

        // TODO: consider making this smarter for orders mainnet orders shortly
        // before the deadline and L2s in general
        let deadline = self.eth.current_block().borrow().number + 1;

        let request = settle::Request {
            auction_id: fast_path_data.auction_id,
            solution_id: fast_path_data.solution_id,
            submission_deadline_latest_block: deadline,
            fast_path: Some(settle::FastPath {
                order: infra::persistence::dto::order::from_domain(&domain_order),
                limit_prices: settle::LimitPrices {
                    sell: limit_sell,
                    buy: limit_buy,
                },
                native_prices: fast_path_data.native_prices.clone(),
            }),
        };

        let res = self
            .settle_coordinator
            .settle(
                winner,
                winner.submission_address,
                fast_path_data.solution_uid,
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
    /// auction and rewrites every bid on the order's
    /// `proposed_trade_executions` row so its
    /// `executed_sell`/`executed_buy` reflect fees that will actually be
    /// captured at settlement.
    ///
    /// For fast-path we can only *apply* Volume-type policies to the quoted
    /// amounts — Surplus and PriceImprovement need an execution-vs-quote
    /// comparison that doesn't exist here — but Surplus / PriceImprovement
    /// policies are still recorded so downstream accounting reflects reality
    /// if they ever become applicable.
    async fn compute_and_persist_fees(
        &self,
        fast_path_data: &FastPathOrder,
    ) -> anyhow::Result<AppliedFees> {
        let order_uid: domain::OrderUid = fast_path_data.model_order.metadata.uid.into();
        // `raw_sell`/`raw_buy` are the placeholder `proposed_trade_executions`
        // amounts written at quote time; they equal the quote's own amounts
        // because nothing rewrites them between then and now.
        let quote = domain::Quote {
            order_uid,
            sell_amount: fast_path_data.raw_sell.into(),
            buy_amount: fast_path_data.raw_buy.into(),
            // The synthetic competition doesn't carry a network fee — the
            // solver's quoted amounts already include everything the user
            // will pay. Represent that as a zero fee here.
            fee: U256::ZERO.into(),
            solver: fast_path_data.solver.0.into(),
        };
        let policies = self.protocol_fees.apply(
            &fast_path_data.model_order,
            Some(&quote),
            &self.surplus_capturing_jit_order_owners,
        );
        let volume_factors: Vec<_> = policies
            .iter()
            .filter_map(|p| match p {
                domain::fee::Policy::Volume { factor } => Some(*factor),
                _ => None,
            })
            .collect();
        let (limit_sell, limit_buy) = shared::fee::apply_volume_fees(
            fast_path_data.raw_sell,
            fast_path_data.raw_buy,
            fast_path_data.model_order.data.kind,
            volume_factors.iter().copied(),
        );
        self.persistence
            .record_fast_path_fees(
                fast_path_data.auction_id,
                order_uid,
                fast_path_data.model_order.data.kind,
                &volume_factors,
                &policies,
            )
            .await?;
        Ok(AppliedFees {
            policies,
            quote,
            limit_sell,
            limit_buy,
        })
    }
}

/// Output of the fee-policy computation and bid-adjustment step of the
/// fast-path handler.
struct AppliedFees {
    /// Every policy the order would incur in a regular auction — persisted
    /// verbatim.
    policies: Vec<domain::fee::Policy>,
    /// The quote implied by the placeholder trade execution amounts;
    /// forwarded to the driver alongside the placed order.
    quote: domain::Quote,
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
