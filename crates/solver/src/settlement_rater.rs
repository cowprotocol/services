use {
    crate::{
        driver::solver_settlements::RatedSettlement,
        settlement::Settlement,
        settlement_access_list::{estimate_settlement_access_list, AccessListEstimating},
        settlement_simulation::{
            call_data,
            settle_method,
            simulate_and_estimate_gas_at_current_block,
        },
        settlement_submission::gas_limit_for_estimate,
        solver::{SettlementWithSolver, Simulation, SimulationError, SimulationWithError, Solver},
    },
    anyhow::{Context, Result},
    contracts::GPv2Settlement,
    ethcontract::errors::ExecutionError,
    futures::{future, future::join_all},
    gas_estimation::GasPrice1559,
    itertools::{Either, Itertools},
    model::solver_competition::Score,
    num::BigRational,
    number_conversions::big_rational_to_u256,
    primitive_types::U256,
    shared::{
        code_fetching::CodeFetching,
        ethrpc::Web3,
        external_prices::ExternalPrices,
        http_solver::model::{InternalizationStrategy, SimulatedTransaction},
    },
    std::{
        borrow::Borrow,
        collections::{HashMap, HashSet},
        sync::Arc,
    },
    web3::types::AccessList,
};

type SolverSettlement = (Arc<dyn Solver>, Settlement);
pub type RatedSolverSettlement = (Arc<dyn Solver>, RatedSettlement, Option<AccessList>);

pub struct SimulationWithResult {
    pub simulation: Simulation,
    /// The outcome of the simulation. Contains either how much gas the
    /// settlement used or the reason why the transaction reverted during
    /// the simulation.
    pub gas_estimate: Result<U256, ExecutionError>,
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait SettlementRating: Send + Sync {
    /// Rate settlements, ignoring those for which the rating procedure failed.
    async fn rate_settlements(
        &self,
        settlements: Vec<SolverSettlement>,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SimulationWithError>)>;
}

pub struct SettlementRater {
    pub access_list_estimator: Arc<dyn AccessListEstimating>,
    pub code_fetcher: Arc<dyn CodeFetching>,
    pub settlement_contract: GPv2Settlement,
    pub web3: Web3,
}

impl SettlementRater {
    async fn append_access_lists(
        &self,
        solver_settlements: Vec<(Arc<dyn Solver>, Settlement)>,
        gas_price: GasPrice1559,
        internalization: InternalizationStrategy,
    ) -> Vec<SettlementWithSolver> {
        join_all(
            solver_settlements
                .into_iter()
                .map(|(solver, settlement)| async {
                    let tx = settle_method(
                        gas_price,
                        &self.settlement_contract,
                        settlement.clone().encode(internalization),
                        solver.account().clone(),
                    )
                    .tx;
                    let access_list = estimate_settlement_access_list(
                        self.access_list_estimator.borrow(),
                        self.code_fetcher.borrow(),
                        self.web3.clone(),
                        solver.account().clone(),
                        &settlement,
                        &tx,
                    )
                    .await
                    .ok();
                    (solver, settlement, access_list)
                }),
        )
        .await
    }

    /// Simulates the settlements and returns the gas used (or reason for
    /// revert) as well as the access list for each settlement.
    async fn simulate_settlements(
        &self,
        settlements: Vec<(Arc<dyn Solver>, Settlement)>,
        gas_price: GasPrice1559,
        internalization: InternalizationStrategy,
    ) -> Result<Vec<SimulationWithResult>> {
        let settlements = self
            .append_access_lists(settlements, gas_price, internalization)
            .await;
        let block_number = self.web3.eth().block_number().await?.as_u64();
        let simulations = simulate_and_estimate_gas_at_current_block(
            settlements.iter().map(|(solver, settlement, access_list)| {
                (
                    solver.account().clone(),
                    settlement.clone().encode(internalization),
                    access_list.clone(),
                )
            }),
            &self.settlement_contract,
            gas_price,
        )
        .await
        .context("failed to simulate settlements")?;

        let details: Vec<_> = settlements
            .into_iter()
            .zip(simulations.into_iter())
            .map(
                |((solver, settlement, access_list), simulation_result)| SimulationWithResult {
                    simulation: Simulation {
                        transaction: SimulatedTransaction {
                            internalization,
                            access_list,
                            block_number,
                            to: self.settlement_contract.address(),
                            from: solver.account().address(),
                            data: call_data(settlement.clone().encode(internalization)),
                            max_fee_per_gas: U256::from_f64_lossy(gas_price.max_fee_per_gas),
                            max_priority_fee_per_gas: U256::from_f64_lossy(
                                gas_price.max_priority_fee_per_gas,
                            ),
                        },
                        settlement,
                        solver,
                    },
                    gas_estimate: simulation_result,
                },
            )
            .collect();
        Ok(details)
    }
}

#[async_trait::async_trait]
impl SettlementRating for SettlementRater {
    async fn rate_settlements(
        &self,
        settlements: Vec<SolverSettlement>,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SimulationWithError>)> {
        // first simulate settlements without internalizations to make sure they pass
        let simulations = self
            .simulate_settlements(
                settlements,
                gas_price,
                InternalizationStrategy::EncodeAllInteractions,
            )
            .await?;

        // split simulations into succeeded and failed groups, then do the rating only
        // for succeeded settlements
        let (settlements, simulations_failed): (Vec<_>, Vec<_>) = simulations
            .into_iter()
            .partition_map(|simulation| match simulation.gas_estimate {
                Ok(_) => Either::Left((
                    simulation.simulation.solver,
                    simulation.simulation.settlement,
                )),
                Err(_) => Either::Right(simulation),
            });

        // since rating is done with internalizations, repeat the simulations for
        // previously succeeded simulations
        let mut simulations = self
            .simulate_settlements(
                settlements,
                gas_price,
                InternalizationStrategy::SkipInternalizableInteraction,
            )
            .await?;

        let effective_gas_price =
            BigRational::from_float(gas_price.effective_gas_price()).expect("Invalid gas price.");

        let rate_settlement = |id, settlement: Settlement, gas_estimate: U256| {
            let earned_fees = settlement.total_earned_fees(prices);
            let inputs = crate::objective_value::Inputs::from_settlement(
                &settlement,
                prices,
                effective_gas_price.clone(),
                &gas_estimate,
            );
            let objective_value = inputs.objective_value();
            let score = match &settlement.score {
                Some(score) => match score {
                    shared::http_solver::model::Score::Score(score) => Score::Solver(*score),
                    shared::http_solver::model::Score::Discount(discount) => Score::Discounted(
                        big_rational_to_u256(&objective_value)
                            .unwrap_or_default()
                            .saturating_sub(*discount),
                    ),
                },
                None => Score::Protocol(big_rational_to_u256(&objective_value).unwrap_or_default()),
            };
            RatedSettlement {
                id,
                settlement,
                surplus: inputs.surplus_given,
                earned_fees,
                solver_fees: inputs.solver_fees,
                gas_estimate,
                gas_price: effective_gas_price.clone(),
                objective_value,
                score,
                ranking: Default::default(),
            }
        };

        let solver_balances = future::join_all(
            simulations
                .iter()
                .filter(|result| result.gas_estimate.is_ok())
                .map(|result| result.simulation.solver.account().address())
                .collect::<HashSet<_>>()
                .into_iter()
                .map(|solver_address| {
                    let web3 = self.web3.clone();
                    async move {
                        (
                            solver_address,
                            web3.eth()
                                .balance(solver_address, None)
                                .await
                                .unwrap_or_default(),
                        )
                    }
                }),
        )
        .await
        .into_iter()
        .collect::<HashMap<_, _>>();

        simulations.extend(simulations_failed);
        Ok((simulations.into_iter().enumerate()).partition_map(
            |(
                i,
                SimulationWithResult {
                    simulation,
                    gas_estimate,
                },
            )| {
                match gas_estimate {
                    Ok(gas_estimate) => {
                        let gas_limit = gas_limit_for_estimate(gas_estimate);
                        let required_balance = gas_limit
                            .saturating_mul(U256::from_f64_lossy(gas_price.max_fee_per_gas));
                        let solver_balance = solver_balances
                            .get(&simulation.solver.account().address())
                            .copied()
                            .unwrap_or_default();

                        if solver_balance >= required_balance {
                            Either::Left((
                                simulation.solver,
                                rate_settlement(i, simulation.settlement, gas_estimate),
                                simulation.transaction.access_list,
                            ))
                        } else {
                            Either::Right(SimulationWithError {
                                simulation,
                                error: SimulationError::InsufficientBalance {
                                    needs: required_balance,
                                    has: solver_balance,
                                },
                            })
                        }
                    }
                    Err(err) => Either::Right(SimulationWithError {
                        simulation,
                        error: err.into(),
                    }),
                }
            },
        ))
    }
}
