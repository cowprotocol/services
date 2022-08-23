use anyhow::{Context, Result};
use ethcontract::Account;
use gas_estimation::GasPriceEstimating;
use model::order::OrderUid;
use num::ToPrimitive;
use number_conversions::big_rational_to_u256;
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use shared::conversions::U256Ext;
use solver::{
    driver_logger::DriverLogger,
    settlement::Settlement,
    settlement_ranker::SettlementRanker,
    solver::{Auction, Solver, SolverRunError},
};
use std::sync::{Arc, Mutex};

/// A `SolutionSummary` holds all information solvers are willing to disclose during settlement
/// competition. It does **not** have to include the call data, yet.
#[derive(Debug, Default, Clone, Deserialize, Serialize, PartialEq)]
pub struct SettlementSummary {
    /// Surplus is denominated in the chain's native token and based off of the auction's external
    /// prices.
    pub surplus: f64,
    /// This is how much gas the solver would like to get reimbursed for executing this solution.
    pub gas_reimbursement: U256,
    /// Orders which would get settled by this solution. Partially fillable orders don't have to be
    /// filled completely to be considered in this list.
    pub settled_orders: Vec<OrderUid>,
}

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub trait CommitRevealSolving: Send + Sync {
    /// Calculates a solution for a given `Auction` but does **not** disclose secret details.
    async fn commit(&self, auction: Auction) -> Result<SettlementSummary>;

    /// Finalizes solution for a previously calculated `SolutionSummary` which can be used to compute
    /// executable call data. If the solver no longer wants to execute the solution it returns
    /// `Ok(None)`.
    async fn reveal(&self, summary: SettlementSummary) -> Result<Option<Settlement>>;

    fn account(&self) -> &Account;

    fn name(&self) -> &str;
}

// Wraps a legacy `Solver` implementation and makes it compatible with the commit reveal protocol.
// Because RFQ support can not be solved generically the wrapped `Solver` will not be able to opt into
// RFQ orders, yet. A solver would have to support RFQ themselves.
// For now this wrapper is only a compatibility layer to let us use the new driver with existing
// solvers for faster development.
pub struct CommitRevealSolver {
    solver: Arc<dyn Solver>,
    gas_estimator: Arc<dyn GasPriceEstimating>,
    stored_solution: Mutex<Option<(SettlementSummary, Settlement)>>,
    settlement_ranker: Arc<SettlementRanker>,
    logger: Arc<DriverLogger>,
}

impl CommitRevealSolver {
    pub fn new(
        solver: Arc<dyn Solver>,
        gas_estimator: Arc<dyn GasPriceEstimating>,
        settlement_ranker: Arc<SettlementRanker>,
        logger: Arc<DriverLogger>,
    ) -> Self {
        Self {
            solver,
            gas_estimator,
            stored_solution: Mutex::new(Default::default()),
            settlement_ranker,
            logger,
        }
    }

    async fn commit_impl(&self, auction: Auction) -> Result<(SettlementSummary, Settlement)> {
        let prices = auction.external_prices.clone();
        let liquidity_fetch_block = auction.liquidity_fetch_block;
        let solutions = match tokio::time::timeout_at(
            auction.deadline.into(),
            self.solver.solve(auction),
        )
        .await
        {
            Ok(inner) => inner.map_err(SolverRunError::Solving),
            Err(_timeout) => Err(SolverRunError::Timeout),
        };

        tracing::debug!(?solutions, "received solutions");

        let gas_price = self.gas_estimator.estimate().await?;
        let (mut rated_settlements, errors) = self
            .settlement_ranker
            .rank_legal_settlements(vec![(self.solver.clone(), solutions)], &prices, gas_price)
            .await?;

        self.logger
            .report_simulation_errors(errors, liquidity_fetch_block, gas_price);

        let (_, winning_settlement, _) = rated_settlements
            .pop()
            .context("could not compute a valid solution")?;

        let summary = SettlementSummary {
            surplus: winning_settlement
                .surplus
                .to_f64()
                .context("couldn't convert surplus to f64")?,
            gas_reimbursement: big_rational_to_u256(
                &(winning_settlement.gas_estimate.to_big_rational() * winning_settlement.gas_price),
            )?,
            settled_orders: winning_settlement
                .settlement
                .traded_orders()
                .map(|order| order.metadata.uid)
                .collect(),
        };

        Ok((summary, winning_settlement.settlement))
    }
}

#[async_trait::async_trait]
impl CommitRevealSolving for CommitRevealSolver {
    async fn commit(&self, auction: Auction) -> Result<SettlementSummary> {
        let result = self.commit_impl(auction).await;
        let mut stored_solution = self.stored_solution.lock().unwrap();

        match result {
            Ok((summary, settlement)) => {
                *stored_solution = Some((summary.clone(), settlement));
                Ok(summary)
            }
            Err(err) => {
                // unset stored_solution so we are not able to reveal an outdated solution by accident
                *stored_solution = None;
                Err(err)
            }
        }
    }

    async fn reveal(&self, summary: SettlementSummary) -> Result<Option<Settlement>> {
        match &*self.stored_solution.lock().unwrap() {
            Some(stored_solution) if stored_solution.0 == summary => {
                // A solver could opt-out of executing the solution but since this is just a component
                // wrapping solvers which don't yet implement the commit-reveal scheme natively we
                // have no way of knowing if the solver would still execute the solution.
                // That's why we will always chose to execute the solution.
                Ok(Some(stored_solution.1.clone()))
            }
            _ => Err(anyhow::anyhow!(
                "could not find solution for requested summary"
            )),
        }
    }

    fn account(&self) -> &Account {
        self.solver.account()
    }

    fn name(&self) -> &str {
        self.solver.name()
    }
}

/// This is just a wrapper type to make a `dyn CommitRevealSolving` usable where `dyn Solver` is
/// expected for logging purposes. This type is only supposed to give information about the
/// name and account of the underlying solver and will panic if `solve()` gets called.
/// Eventually this wrapper should get removed when the logging code got refactored to expect
/// something like a `NamedAccount` (name + account info) instead of an `Arc<dyn Solver>`.
#[derive(Clone)]
pub struct CommitRevealSolverAdapter {
    solver: Arc<dyn CommitRevealSolving>,
}

impl From<Arc<dyn CommitRevealSolving>> for CommitRevealSolverAdapter {
    fn from(solver: Arc<dyn CommitRevealSolving>) -> Self {
        Self { solver }
    }
}

#[async_trait::async_trait]
impl Solver for CommitRevealSolverAdapter {
    async fn solve(&self, _auction: Auction) -> Result<Vec<Settlement>> {
        panic!(
            "A dyn Solver created from a dyn CommitRevealSolving\
            is only supposed to be used for its account data and name."
        )
    }

    fn account(&self) -> &Account {
        self.solver.account()
    }

    fn name(&self) -> &str {
        self.solver.name()
    }
}
