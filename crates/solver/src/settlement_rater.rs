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
        solver::{SettlementWithSolver, Simulation, SimulationWithError, Solver},
    },
    anyhow::{Context, Result},
    contracts::GPv2Settlement,
    ethcontract::errors::ExecutionError,
    futures::future::join_all,
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
    std::{borrow::Borrow, sync::Arc},
    web3::types::AccessList,
};

/// We require from solvers to have a bit more ETH balance then needed
/// at the moment of simulating the transaction, to cover the potential increase
/// of the cost of sending transaction onchain, because of the sudden gas price
/// increase. To simulate this sudden increase of gas price during simulation,
/// we artificially multiply the gas price with this factor.
///
/// We chose the multiplier of 3.25 to be approximately equal to the maximum
/// increase in the ERC-1559 base gas price over 10 blocks, or ~120s. This maps
/// exactly to the timeout we allow for any given transaction.
const SOLVER_BALANCE_MULTIPLIER: f64 = 3.25;

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

    /// Simulates the settlements and returns the gas used (or reason for
    /// revert) as well as the access list for each settlement.
    async fn simulate_settlements(
        &self,
        settlements: Vec<SolverSettlement>,
        gas_price: GasPrice1559,
        internalization: InternalizationStrategy,
    ) -> Result<Vec<SimulationWithResult>>;
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
}

#[async_trait::async_trait]
impl SettlementRating for SettlementRater {
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

    async fn rate_settlements(
        &self,
        settlements: Vec<SolverSettlement>,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SimulationWithError>)> {
        let gas_price_for_simulation = gas_price_for_simulation(&gas_price);

        // first simulate settlements without internalizations to make sure they pass
        let simulations = self
            .simulate_settlements(
                settlements,
                gas_price_for_simulation,
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
                gas_price_for_simulation,
                InternalizationStrategy::SkipInternalizableInteraction,
            )
            .await?;

        let gas_price =
            BigRational::from_float(gas_price.effective_gas_price()).expect("Invalid gas price.");

        let rate_settlement = |id, settlement: Settlement, gas_estimate: U256| {
            let earned_fees = settlement.total_earned_fees(prices);
            let inputs = crate::objective_value::Inputs::from_settlement(
                &settlement,
                prices,
                gas_price.clone(),
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
                gas_price: gas_price.clone(),
                objective_value,
                score,
                ranking: Default::default(),
            }
        };

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
                    Ok(gas_estimate) => Either::Left((
                        simulation.solver,
                        rate_settlement(i, simulation.settlement, gas_estimate),
                        simulation.transaction.access_list,
                    )),
                    Err(err) => Either::Right(SimulationWithError {
                        simulation,
                        error: err,
                    }),
                }
            },
        ))
    }
}

fn gas_price_for_simulation(gas_price: &GasPrice1559) -> GasPrice1559 {
    let bumped_effective_gas_price = gas_price.effective_gas_price() * SOLVER_BALANCE_MULTIPLIER;
    let max_priority_fee_per_gas = bumped_effective_gas_price - gas_price.base_fee_per_gas;

    GasPrice1559 {
        max_fee_per_gas: gas_price.max_fee_per_gas.max(bumped_effective_gas_price),
        max_priority_fee_per_gas,
        base_fee_per_gas: gas_price.base_fee_per_gas,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gas_price_for_simulation_is_bumped() {
        let gas_price = GasPrice1559 {
            max_fee_per_gas: 200.0,
            max_priority_fee_per_gas: 10.0,
            base_fee_per_gas: 90.0,
        };
        let bumped_gas_price = gas_price_for_simulation(&gas_price);
        assert_eq!(
            gas_price.effective_gas_price() * SOLVER_BALANCE_MULTIPLIER,
            bumped_gas_price.effective_gas_price()
        );
    }
}
