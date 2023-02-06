use crate::{
    analytics,
    driver::solver_settlements::RatedSettlement,
    metrics::{SolverMetrics, SolverSimulationOutcome},
    settlement::Settlement,
    settlement_simulation::{
        simulate_and_error_with_tenderly_link, simulate_before_after_access_list,
    },
    settlement_submission::SubmissionError,
    solver::{Simulation, SimulationWithError, Solver},
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use gas_estimation::GasPrice1559;
use itertools::Itertools;
use model::order::{Order, OrderKind};
use num::{BigRational, ToPrimitive};
use primitive_types::H256;
use shared::{ethrpc::Web3, tenderly_api::TenderlyApi};
use std::sync::Arc;
use tracing::{Instrument as _, Span};
use web3::types::{AccessList, TransactionReceipt};

pub struct DriverLogger {
    pub metrics: Arc<dyn SolverMetrics>,
    pub web3: Web3,
    pub tenderly: Option<Arc<dyn TenderlyApi>>,
    pub network_id: String,
    pub settlement_contract: GPv2Settlement,
    pub simulation_gas_limit: u128,
}

impl DriverLogger {
    pub async fn metric_access_list_gas_saved(&self, transaction_hash: H256) -> Result<()> {
        let gas_saved = simulate_before_after_access_list(
            &self.web3,
            self.tenderly.as_deref().context("tenderly disabled")?,
            self.network_id.clone(),
            transaction_hash,
        )
        .await?;
        tracing::debug!(?gas_saved, "access list gas saved");
        if gas_saved.is_sign_positive() {
            self.metrics
                .settlement_access_list_saved_gas(gas_saved, "positive");
        } else {
            self.metrics
                .settlement_access_list_saved_gas(-gas_saved, "negative");
        }

        Ok(())
    }

    /// Collects all orders which got traded in the settlement. Tapping into partially fillable
    /// orders multiple times will not result in duplicates. Partially fillable orders get
    /// considered as traded only the first time we tap into their liquidity.
    fn get_traded_orders(settlement: &Settlement) -> Vec<Order> {
        let mut traded_orders = Vec::new();
        for (_, group) in &settlement
            .trades()
            .group_by(|trade| trade.order.metadata.uid)
        {
            let mut group = group.into_iter().peekable();
            let order = &group.peek().unwrap().order;
            let was_already_filled = match order.data.kind {
                OrderKind::Buy => &order.metadata.executed_buy_amount,
                OrderKind::Sell => &order.metadata.executed_sell_amount,
            } > &0u8.into();
            let is_getting_filled = group.any(|trade| !trade.executed_amount.is_zero());
            if !was_already_filled && is_getting_filled {
                traded_orders.push(order.clone());
            }
        }
        traded_orders
    }

    pub async fn log_submission_info(
        &self,
        submission: &Result<TransactionReceipt, SubmissionError>,
        settlement: &Settlement,
        settlement_id: Option<u64>,
        solver_name: &str,
    ) {
        self.metrics
            .settlement_revertable_status(settlement.revertable(), solver_name);
        match submission {
            Ok(receipt) => {
                tracing::info!(
                    settlement_id,
                    transaction_hash =? receipt.transaction_hash,
                    "Successfully submitted settlement",
                );
                Self::get_traded_orders(settlement)
                    .iter()
                    .for_each(|order| self.metrics.order_settled(order, solver_name));
                self.metrics.settlement_submitted(
                    crate::metrics::SettlementSubmissionOutcome::Success,
                    solver_name,
                );
                if let Err(err) = self
                    .metric_access_list_gas_saved(receipt.transaction_hash)
                    .await
                {
                    tracing::debug!(?err, "access list metric not saved");
                }
                match receipt.effective_gas_price {
                    Some(price) => {
                        self.metrics.transaction_gas_price(price);
                    }
                    None => {
                        tracing::error!("node did not return effective gas price in tx receipt");
                    }
                }
            }
            Err(err) => {
                // Since we simulate and only submit solutions when they used to pass before, there is no
                // point in logging transaction failures in the form of race conditions as hard errors.
                tracing::warn!(settlement_id, ?err, "Failed to submit settlement",);
                self.metrics
                    .settlement_submitted(err.as_outcome(), solver_name);
                if let Some(transaction_hash) = err.transaction_hash() {
                    if let Err(err) = self.metric_access_list_gas_saved(transaction_hash).await {
                        tracing::debug!(?err, "access list metric not saved");
                    }
                }
            }
        }
    }

    // Log simulation errors only if the simulation also fails in the block at which on chain
    // liquidity was queried. If the simulation succeeds at the previous block then the solver
    // worked correctly and the error doesn't have to be reported.
    // Note that we could still report a false positive because the earlier block might be off by if
    // the block has changed just as were were querying the node.
    pub fn report_simulation_errors(
        &self,
        errors: Vec<SimulationWithError>,
        current_block_during_liquidity_fetch: u64,
        gas_price: GasPrice1559,
    ) {
        let contract = self.settlement_contract.clone();
        let web3 = self.web3.clone();
        let network_id = self.network_id.clone();
        let metrics = self.metrics.clone();
        let simulation_gas_limit = self.simulation_gas_limit;
        let task = async move {
            let simulations = simulate_and_error_with_tenderly_link(
                errors.iter().map(|simulation_with_error| {
                    let simulation = &simulation_with_error.simulation;
                    let settlement = simulation
                        .settlement
                        .clone()
                        .encode(simulation.transaction.internalization);
                    (
                        simulation.solver.account().clone(),
                        settlement,
                        simulation.transaction.access_list.clone(),
                    )
                }),
                &contract,
                &web3,
                gas_price,
                &network_id,
                current_block_during_liquidity_fetch,
                simulation_gas_limit,
            )
            .await;

            for (
                SimulationWithError {
                    simulation:
                        Simulation {
                            solver, settlement, ..
                        },
                    error: error_at_latest_block,
                },
                result,
            ) in errors.iter().zip(simulations)
            {
                metrics
                    .settlement_simulation(solver.name(), SolverSimulationOutcome::FailureOnLatest);
                if let Err(error_at_earlier_block) = result {
                    tracing::warn!(
                        "{} settlement simulation failed at submission and block {}:\n{:?}",
                        solver.name(),
                        current_block_during_liquidity_fetch,
                        error_at_earlier_block,
                    );
                    // split warning into separate logs so that the messages aren't too long.
                    tracing::warn!(
                        "{} settlement failure for: \n{:#?}",
                        solver.name(),
                        settlement,
                    );

                    metrics.settlement_simulation(solver.name(), SolverSimulationOutcome::Failure);
                } else {
                    tracing::debug!(
                        name = solver.name(),
                        ?error_at_latest_block,
                        "simulation only failed on the latest block but not on the block the auction started",
                    );
                }
            }
        };
        tokio::task::spawn(task.instrument(Span::current()));
    }

    pub fn print_settlements(
        rated_settlements: &[(Arc<dyn Solver>, RatedSettlement, Option<AccessList>)],
        fee_objective_scaling_factor: &BigRational,
    ) {
        let mut text = String::new();
        for (solver, settlement, access_list) in rated_settlements {
            use std::fmt::Write;
            write!(
                text,
                "\nid={} solver={} \
             objective={:.2e} surplus={:.2e} \
             gas_estimate={:.2e} gas_price={:.2e} \
             unscaled_unsubsidized_fee={:.2e} unscaled_subsidized_fee={:.2e} \
             access_list_addreses={}",
                settlement.id,
                solver.name(),
                settlement.objective_value.to_f64().unwrap_or(f64::NAN),
                settlement.surplus.to_f64().unwrap_or(f64::NAN),
                settlement.gas_estimate.to_f64_lossy(),
                settlement.gas_price.to_f64().unwrap_or(f64::NAN),
                (&settlement.scaled_unsubsidized_fee / fee_objective_scaling_factor)
                    .to_f64()
                    .unwrap_or(f64::NAN),
                settlement
                    .unscaled_subsidized_fee
                    .to_f64()
                    .unwrap_or(f64::NAN),
                access_list.clone().unwrap_or_default().len()
            )
            .unwrap();
        }
        tracing::info!("Rated Settlements: {}", text);
    }

    /// Record metrics on the matched orders from a single batch. Specifically we report on
    /// the number of orders that were;
    ///  - surplus in winning settlement vs unrealized surplus from other feasible solutions.
    ///  - matched but not settled in this runloop (effectively queued for the next one)
    /// Should help us to identify how much we can save by parallelizing execution.
    pub fn report_on_batch(
        &self,
        submitted: &(Arc<dyn Solver>, RatedSettlement),
        other_settlements: Vec<(Arc<dyn Solver>, RatedSettlement)>,
    ) {
        // Report surplus
        analytics::report_alternative_settlement_surplus(
            &*self.metrics,
            submitted,
            &other_settlements,
        );
        // Report matched but not settled
        analytics::report_matched_but_not_settled(&*self.metrics, submitted, &other_settlements);
    }
}

#[cfg(test)]
mod tests {
    use model::solver_competition::Score;

    use super::*;
    use crate::solver::dummy_arc_solver;

    #[test]
    #[ignore]
    fn print_settlements() {
        let a = [
            (
                dummy_arc_solver(),
                RatedSettlement {
                    id: 0,
                    settlement: Default::default(),
                    surplus: BigRational::new(1u8.into(), 1u8.into()),
                    unscaled_subsidized_fee: BigRational::new(2u8.into(), 1u8.into()),
                    scaled_unsubsidized_fee: BigRational::new(3u8.into(), 1u8.into()),
                    gas_estimate: 4.into(),
                    gas_price: BigRational::new(5u8.into(), 1u8.into()),
                    objective_value: BigRational::new(6u8.into(), 1u8.into()),
                    score: Score::Solver(6.),
                    ranking: 1,
                },
                None,
            ),
            (
                dummy_arc_solver(),
                RatedSettlement {
                    id: 6,
                    settlement: Default::default(),
                    surplus: BigRational::new(7u8.into(), 1u8.into()),
                    unscaled_subsidized_fee: BigRational::new(8u8.into(), 1u8.into()),
                    scaled_unsubsidized_fee: BigRational::new(9u8.into(), 1u8.into()),
                    gas_estimate: 10.into(),
                    gas_price: BigRational::new(11u8.into(), 1u8.into()),
                    objective_value: BigRational::new(12u8.into(), 1u8.into()),
                    score: Score::Solver(12.),
                    ranking: 2,
                },
                None,
            ),
        ];

        shared::tracing::initialize_for_tests("INFO");
        DriverLogger::print_settlements(&a, &BigRational::new(1u8.into(), 2u8.into()));
    }
}
