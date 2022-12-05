use crate::{
    driver::solver_settlements::RatedSettlement,
    settlement::{external_prices::ExternalPrices, Settlement},
    settlement_access_list::{estimate_settlement_access_list, AccessListEstimating},
    settlement_simulation::{call_data, settle_method, simulate_and_estimate_gas_at_current_block},
    solver::{SettlementWithSolver, Simulation, SimulationWithError, Solver},
};
use anyhow::{Context, Result};
use contracts::GPv2Settlement;
use ethcontract::{errors::ExecutionError, H160};
use futures::future::join_all;
use gas_estimation::GasPrice1559;
use itertools::{Either, Itertools};
use num::BigRational;
use primitive_types::U256;
use shared::{
    code_fetching::CodeFetching,
    ethrpc::Web3,
    http_solver::model::{InternalizationStrategy, SimulatedTransaction},
};
use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet},
    sync::Arc,
};
use web3::types::AccessList;

type SolverSettlement = (Arc<dyn Solver>, Settlement);
pub type RatedSolverSettlement = (Arc<dyn Solver>, RatedSettlement, Option<AccessList>);

pub struct SimulationWithResult {
    pub simulation: Simulation,
    /// The outcome of the simulation. Contains either how much gas the settlement used or the
    /// reason why the transaction reverted during the simulation.
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

    /// Simulates the settlements and returns the gas used (or reason for revert) as well as
    /// the access list for each settlement.
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
        // first simulate settlements without internalizations to make sure they pass
        let simulations = self
            .simulate_settlements(
                settlements,
                gas_price,
                InternalizationStrategy::EncodeAllInteractions,
            )
            .await?;

        // split simulations into succeeded and failed groups, then do the rating only for succeeded settlements
        let (settlements, simulations_failed): (Vec<_>, Vec<_>) = simulations
            .into_iter()
            .partition_map(|simulation| match simulation.gas_estimate {
                Ok(_) => Either::Left((
                    simulation.simulation.solver,
                    simulation.simulation.settlement,
                )),
                Err(_) => Either::Right(simulation),
            });

        // since rating is done with internalizations, repeat the simulations for previously succeeded simulations
        let mut simulations = self
            .simulate_settlements(
                settlements,
                gas_price,
                InternalizationStrategy::SkipInternalizableInteraction,
            )
            .await?;

        let solver_addresses = simulations
            .iter()
            .filter_map(|simulation| match simulation.gas_estimate {
                Ok(_) => Some(simulation.simulation.solver.account().address()),
                Err(_) => None,
            })
            .collect::<HashSet<_>>();
        let solver_balances = solver_balances(&self.web3, solver_addresses).await?;

        let gas_price =
            BigRational::from_float(gas_price.effective_gas_price()).expect("Invalid gas price.");

        let rate_settlement = |id, settlement: Settlement, gas_estimate, solver_balance| {
            let surplus = settlement.total_surplus(prices);
            let scaled_solver_fees = settlement.total_scaled_unsubsidized_fees(prices);
            let unscaled_subsidized_fee = settlement.total_unscaled_subsidized_fees(prices);
            RatedSettlement {
                id,
                settlement,
                surplus,
                unscaled_subsidized_fee,
                scaled_unsubsidized_fee: scaled_solver_fees,
                gas_estimate,
                gas_price: gas_price.clone(),
                solver_balance,
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
                    Ok(gas_estimate) => {
                        let solver_balance = solver_balances
                            .get(&simulation.solver.account().address())
                            .cloned()
                            .expect("missing solver balance");
                        Either::Left((
                            simulation.solver,
                            rate_settlement(i, simulation.settlement, gas_estimate, solver_balance),
                            simulation.transaction.access_list,
                        ))
                    }
                    Err(err) => Either::Right(SimulationWithError {
                        simulation,
                        error: err,
                    }),
                }
            },
        ))
    }
}

async fn solver_balances(web3: &Web3, addresses: HashSet<H160>) -> Result<HashMap<H160, U256>> {
    let addresses: Vec<H160> = addresses.into_iter().collect();
    let futures = addresses
        .iter()
        .map(|address| web3.eth().balance(*address, None))
        .collect::<Vec<_>>();

    futures::future::join_all(futures)
        .await
        .into_iter()
        .enumerate()
        .map(|(index, result)| match result {
            Ok(balance) => Ok((addresses[index], balance)),
            Err(err) => Err(err.into()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use shared::{addr, ethrpc::create_env_test_transport};

    #[tokio::test]
    #[ignore]
    async fn solver_balances_test() {
        let transport = create_env_test_transport();
        let web3 = Web3::new(transport);

        let addreses = HashSet::from([
            addr!("731a0A8ab2C6FcaD841e82D06668Af7f18e34970"),
            addr!("b20B86C4e6DEEB432A22D773a221898bBBD03036"),
            addr!("b20B86C4e6DEEB432A22D773a221898bBBD03036"),
        ]);
        let balances = solver_balances(&web3, addreses).await.unwrap();
        dbg!(balances.clone());
        assert_eq!(balances.len(), 2);
    }
}
