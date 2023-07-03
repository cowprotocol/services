use {
    crate::{
        driver::solver_settlements::RatedSettlement,
        settlement::{self, Settlement},
        settlement_access_list::{estimate_settlement_access_list, AccessListEstimating},
        settlement_simulation::{
            call_data,
            settle_method,
            simulate_and_estimate_gas_at_current_block,
        },
        settlement_submission::gas_limit_for_estimate,
        solver::{Simulation, SimulationError, SimulationWithError, SolverInfo},
    },
    anyhow::{Context, Result},
    contracts::GPv2Settlement,
    ethcontract::{errors::ExecutionError, Account},
    gas_estimation::GasPrice1559,
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

struct SimulationSuccess {
    pub simulation: Simulation,
    pub gas_estimate: U256,
}

struct SimulationFailure {
    pub simulation: Simulation,
    pub error: ExecutionError,
}

enum SimulationResult {
    Ok(SimulationSuccess),
    Err(SimulationFailure),
}

pub enum Rating {
    Ok(RatedSettlement),
    Err(SimulationWithError),
}

#[mockall::automock]
#[async_trait::async_trait]
pub trait SettlementRating: Send + Sync {
    async fn rate_settlement(
        &self,
        solver: &SolverInfo,
        settlement: Settlement,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
        id: usize,
    ) -> Result<Rating>;
}

pub struct SettlementRater {
    pub access_list_estimator: Arc<dyn AccessListEstimating>,
    pub code_fetcher: Arc<dyn CodeFetching>,
    pub settlement_contract: GPv2Settlement,
    pub settlement_encoding_contracts: settlement::Contracts,
    pub web3: Web3,
}

impl SettlementRater {
    async fn generate_access_list(
        &self,
        account: &Account,
        settlement: &Settlement,
        gas_price: GasPrice1559,
        internalization: InternalizationStrategy,
    ) -> Option<AccessList> {
        let tx = settle_method(
            gas_price,
            &self.settlement_contract,
            settlement
                .clone()
                .encode(&self.settlement_encoding_contracts, internalization),
            account.clone(),
        )
        .tx;
        estimate_settlement_access_list(
            self.access_list_estimator.borrow(),
            self.code_fetcher.borrow(),
            self.web3.clone(),
            account.clone(),
            settlement,
            &tx,
        )
        .await
        .ok()
    }

    /// Simulates the settlement and returns the gas used or the reason for a
    /// revert.
    async fn simulate_settlement(
        &self,
        solver: &SolverInfo,
        settlement: &Settlement,
        gas_price: GasPrice1559,
        internalization: InternalizationStrategy,
    ) -> Result<SimulationResult> {
        let access_list = self
            .generate_access_list(&solver.account, settlement, gas_price, internalization)
            .await;
        let block_number = self.web3.eth().block_number().await?.as_u64();
        let simulation_result = simulate_and_estimate_gas_at_current_block(
            std::iter::once((
                solver.account.clone(),
                settlement
                    .clone()
                    .encode(&self.settlement_encoding_contracts, internalization),
                access_list.clone(),
            )),
            &self.settlement_contract,
            gas_price,
        )
        .await
        .context("failed to simulate settlements")?
        .pop()
        .expect("yields exactly 1 item");

        let simulation = Simulation {
            transaction: SimulatedTransaction {
                internalization,
                access_list,
                // simulating on block X and tx index A is equal to simulating on block
                // X+1 and tx index 0.
                block_number: block_number + 1,
                tx_index: 0,
                to: self.settlement_contract.address(),
                from: solver.account.address(),
                data: call_data(
                    settlement
                        .clone()
                        .encode(&self.settlement_encoding_contracts, internalization),
                ),
                max_fee_per_gas: U256::from_f64_lossy(gas_price.max_fee_per_gas),
                max_priority_fee_per_gas: U256::from_f64_lossy(gas_price.max_priority_fee_per_gas),
            },
            settlement: settlement.clone(),
            solver: solver.clone(),
        };

        let result = match simulation_result {
            Ok(gas_estimate) => SimulationResult::Ok(SimulationSuccess {
                simulation,
                gas_estimate,
            }),
            Err(error) => SimulationResult::Err(SimulationFailure { simulation, error }),
        };
        Ok(result)
    }
}

#[async_trait::async_trait]
impl SettlementRating for SettlementRater {
    async fn rate_settlement(
        &self,
        solver: &SolverInfo,
        settlement: Settlement,
        prices: &ExternalPrices,
        gas_price: GasPrice1559,
        id: usize,
    ) -> Result<Rating> {
        // first simulate settlements without internalizations to make sure they pass
        let simulation_result = self
            .simulate_settlement(
                solver,
                &settlement,
                gas_price,
                InternalizationStrategy::EncodeAllInteractions,
            )
            .await?;

        if let SimulationResult::Err(err) = simulation_result {
            return Ok(Rating::Err(SimulationWithError {
                simulation: err.simulation,
                error: err.error.into(),
            }));
        }

        // since rating is done with internalizations, repeat the simulations for
        // previously succeeded simulations
        let simulation_result = self
            .simulate_settlement(
                solver,
                &settlement,
                gas_price,
                InternalizationStrategy::SkipInternalizableInteraction,
            )
            .await?;

        let simulation = match simulation_result {
            SimulationResult::Ok(success) => success,
            SimulationResult::Err(err) => {
                return Ok(Rating::Err(SimulationWithError {
                    simulation: err.simulation,
                    error: err.error.into(),
                }))
            }
        };

        let effective_gas_price =
            BigRational::from_float(gas_price.effective_gas_price()).expect("Invalid gas price.");

        let solver_balance = self
            .web3
            .eth()
            .balance(solver.account.address(), None)
            .await
            .unwrap_or_default();

        let gas_limit = gas_limit_for_estimate(simulation.gas_estimate);
        let required_balance =
            gas_limit.saturating_mul(U256::from_f64_lossy(gas_price.max_fee_per_gas));

        if solver_balance < required_balance {
            return Ok(Rating::Err(SimulationWithError {
                simulation: simulation.simulation,
                error: SimulationError::InsufficientBalance {
                    needs: required_balance,
                    has: solver_balance,
                },
            }));
        }

        let earned_fees = settlement.total_earned_fees(prices);
        let inputs = crate::objective_value::Inputs::from_settlement(
            &settlement,
            prices,
            effective_gas_price.clone(),
            &simulation.gas_estimate,
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

        let rated_settlement = RatedSettlement {
            id,
            settlement,
            surplus: inputs.surplus_given,
            earned_fees,
            solver_fees: inputs.solver_fees,
            gas_estimate: simulation.gas_estimate,
            gas_price: effective_gas_price.clone(),
            objective_value,
            score,
            ranking: Default::default(),
        };
        Ok(Rating::Ok(rated_settlement))
    }
}
