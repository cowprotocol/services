use {
    crate::{
        arguments::TransactionStrategyArg,
        driver::solver_settlements::{GasCost, RatedSettlement},
        settlement::Settlement,
        settlement_access_list::{estimate_settlement_access_list, AccessListEstimating},
        settlement_simulation::{
            call_data,
            settle_method,
            simulate_and_estimate_gas_at_current_block,
        },
        settlement_submission::gas_limit_for_estimate,
        solver::{Simulation, SimulationError, SimulationWithError, SolverInfo},
    },
    anyhow::{anyhow, ensure, Context, Result},
    contracts::GPv2Settlement,
    ethcontract::{errors::ExecutionError, Account},
    gas_estimation::GasPrice1559,
    model::solver_competition::Score,
    num::{zero, BigRational, CheckedDiv, One, Zero},
    number_conversions::{big_rational_to_u256, u256_to_big_rational},
    primitive_types::U256,
    shared::{
        code_fetching::CodeFetching,
        ethrpc::Web3,
        external_prices::ExternalPrices,
        http_solver::model::{InternalizationStrategy, SimulatedTransaction},
    },
    std::{borrow::Borrow, cmp::min, sync::Arc},
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
    pub web3: Web3,
    pub score_calculator: ScoreCalculator,
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
            settlement.clone().encode(internalization),
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
                settlement.clone().encode(internalization),
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
                data: call_data(settlement.clone().encode(internalization)),
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
        let inputs = {
            let gas_cost = match settlement.gas_cost.as_ref() {
                Some(gas_cost) => number_conversions::u256_to_big_rational(gas_cost),
                None => {
                    number_conversions::u256_to_big_rational(&simulation.gas_estimate)
                        * effective_gas_price.clone()
                }
            };
            crate::objective_value::Inputs::from_settlement(&settlement, prices, gas_cost)
        };
        let objective_value = inputs.objective_value();
        let score = match &settlement.score {
            Some(score) => match score {
                shared::http_solver::model::Score::Solver(score) => Score::Solver(*score),
                shared::http_solver::model::Score::Discount(discount) => Score::Discounted(
                    big_rational_to_u256(&objective_value)
                        .unwrap_or_default()
                        .saturating_sub(*discount),
                ),
            },
            None => Score::Protocol(big_rational_to_u256(&objective_value).unwrap_or_default()),
        };

        // recalculate score if success probability is provided
        let score = match settlement.success_probability {
            Some(success_probability) => {
                match self.score_calculator.compute_score(
                    &objective_value,
                    &inputs.gas_cost,
                    success_probability,
                ) {
                    Ok(score) => score,
                    Err(err) => {
                        tracing::warn!(?err, "Failed to compute score with success probability");
                        score
                    }
                }
            }
            None => score,
        };

        let gas_cost = match settlement.gas_cost.as_ref() {
            Some(gas_cost) => GasCost::SolverEstimated(u256_to_big_rational(gas_cost)),
            None => {
                let gas_cost =
                    &u256_to_big_rational(&simulation.gas_estimate) * &effective_gas_price;
                GasCost::ProtocolEstimated(gas_cost)
            }
        };

        let rated_settlement = RatedSettlement {
            id,
            settlement,
            surplus: inputs.surplus_given,
            earned_fees,
            solver_fees: inputs.solver_fees,
            gas_estimate: simulation.gas_estimate,
            gas_price: effective_gas_price,
            gas_cost,
            objective_value,
            score,
            ranking: Default::default(),
        };
        Ok(Rating::Ok(rated_settlement))
    }
}

pub struct ScoreCalculator {
    score_cap: BigRational,
    submission_strategies: Vec<TransactionStrategyArg>,
}

impl ScoreCalculator {
    pub fn new(score_cap: BigRational, submission_strategies: Vec<TransactionStrategyArg>) -> Self {
        Self {
            score_cap,
            submission_strategies,
        }
    }

    pub fn cost_fail(&self, gas_cost: &BigRational) -> BigRational {
        if self.submission_strategies == vec![TransactionStrategyArg::Flashbots] {
            BigRational::zero()
        } else {
            gas_cost.clone()
        }
    }

    pub fn compute_score(
        &self,
        objective_value: &BigRational,
        gas_cost: &BigRational,
        success_probability: f64,
    ) -> Result<Score> {
        ensure!(
            (0.0..=1.0).contains(&success_probability),
            "success probability must be between 0 and 1."
        );
        let success_probability = BigRational::from_float(success_probability).unwrap();
        let cost_fail = self.cost_fail(gas_cost);
        let optimal_score =
            self.compute_optimal_bid(objective_value.clone(), success_probability, cost_fail)?;
        let score = big_rational_to_u256(&optimal_score).context("Invalid score.")?;
        Ok(Score::ProtocolWithRisk(score))
    }

    fn compute_optimal_bid(
        &self,
        score: BigRational,
        probability_success: BigRational,
        cost_fail: BigRational,
    ) -> Result<BigRational> {
        tracing::trace!(
            ?score,
            ?probability_success,
            ?cost_fail,
            "Computing optimal bid"
        );

        let payout_score_minus_cap = self.payout(
            score.clone() - self.score_cap.clone(),
            score.clone(),
            probability_success.clone(),
            cost_fail.clone(),
        );
        let payout_cap = self.payout(
            self.score_cap.clone(),
            score.clone(),
            probability_success.clone(),
            cost_fail.clone(),
        );
        tracing::trace!(
            ?payout_score_minus_cap,
            ?payout_cap,
            "Payout score minus cap and payout cap"
        );

        let score_cap = self.score_cap.clone();
        let probability_fail = BigRational::one() - probability_success.clone();

        // https://www.notion.so/cownation/Optimal-bidding-strategy-84b4c710466a4c56af9295015308452a
        let bid = {
            if payout_score_minus_cap >= zero() && payout_cap <= zero() {
                // `score - cap <= cap` due to monotonicity of the payout function and the zero
                // of the payout function lies between `score - cap` and `cap`. For such scores
                // the cap in the payment is not binding, the payout function is just linear in
                // the reference score and the zero of the function is at

                // `probability_success * score - probability_fail * cost_fail`
                let bid = probability_success * score.clone() - probability_fail * cost_fail;
                Ok(bid)
            } else if payout_score_minus_cap >= zero() && payout_cap > zero() {
                // optimal score is larger than `score - cap` and `cap` due to monotonicity
                // of the payout function. For such reference scores the cap in the payment is
                // always binding in case of reverts and never in case of success, and the zero
                // of the function is at

                //score - probability_fail / probability_success * (cap + cost_fail)
                let bid = score.clone()
                    - probability_fail
                        .checked_div(&probability_success)
                        .context("division by success")?
                        * (score_cap + cost_fail);
                Ok(bid)
            } else if payout_score_minus_cap < zero() && payout_cap <= zero() {
                // optimal score is smaller than `score - cap` and `cap` due to monotonicity of
                // the payout function. In that case, the cap is always binding in case of
                // success and never binds for reverts. Thus in this range the payout function
                // is again just linear in the reference score and the zero can be computed as

                // probability_success / probability_fail * cap - cost_fail
                let bid = probability_success
                    .checked_div(&probability_fail)
                    .context("division by fail")?
                    * score_cap
                    - cost_fail;
                Ok(bid)
            } else {
                Err(anyhow!("Invalid bid"))
            }
        };

        tracing::trace!(?bid, "Optimal bid");
        if let Ok(bid) = bid.as_ref() {
            ensure!(bid <= &score, "Optimal bid higher than initial score");
        }
        bid
    }

    // amount that should be payed out to solver or penalized from solver
    fn payout(
        &self,
        score_reference: BigRational,
        score: BigRational,
        probability_success: BigRational,
        cost_fail: BigRational,
    ) -> BigRational {
        tracing::trace!(
            ?score_reference,
            ?score,
            ?probability_success,
            ?cost_fail,
            "Computing payout"
        );

        // this much is payed out to the solver if the transaction succeeds
        let reward = min(score - score_reference.clone(), self.score_cap.clone());
        // this much is solver penalized if the transaction fails
        let penalty = min(score_reference, self.score_cap.clone()) + cost_fail;

        tracing::trace!(?reward, ?penalty, "Reward and penalty");

        // final payout is the combination of the two above
        let probability_fail = BigRational::one() - probability_success.clone();
        let payout = probability_success * reward - probability_fail * penalty;

        tracing::trace!(?payout, "Final payout");
        payout
    }
}

#[cfg(test)]
mod tests {
    use {crate::arguments::TransactionStrategyArg, num::BigRational, primitive_types::U256};

    fn calculate_score(
        objective_value: &BigRational,
        gas_cost: &BigRational,
        success_probability: f64,
    ) -> U256 {
        let score_cap = BigRational::from_float(1e16).unwrap();
        let score_calculator =
            super::ScoreCalculator::new(score_cap, vec![TransactionStrategyArg::Flashbots]);
        score_calculator
            .compute_score(objective_value, gas_cost, success_probability)
            .unwrap()
            .score()
    }

    #[test]
    fn compute_score_with_success_probability_test() {
        let objective_value = num::BigRational::from_float(251547381429604400.).unwrap();
        let gas_cost = BigRational::from_float(1e16).unwrap();
        let success_probability = 0.9202405649482063;
        let score =
            calculate_score(&objective_value, &gas_cost, success_probability).to_f64_lossy();
        assert_eq!(score, 250680657682686317.);
    }

    #[test]
    fn compute_score_with_success_probability_test2() {
        let objective_value = num::BigRational::from_float(1e16).unwrap();
        let gas_cost = BigRational::from_float(1e16).unwrap();
        let success_probability = 0.9;
        let score =
            calculate_score(&objective_value, &gas_cost, success_probability).to_f64_lossy();
        assert_eq!(score, 9e15);
    }

    #[test]
    fn compute_score_with_success_probability_test3() {
        let objective_value = num::BigRational::from_float(1e17).unwrap();
        let gas_cost = BigRational::from_float(1e16).unwrap();
        let success_probability = 2.0 / 3.0;
        let score =
            calculate_score(&objective_value, &gas_cost, success_probability).to_f64_lossy();
        assert_eq!(score, 94999999999999999.);
    }

    #[test]
    fn compute_score_with_success_probability_test4() {
        let objective_value = num::BigRational::from_float(1e17).unwrap();
        let gas_cost = BigRational::from_float(1e16).unwrap();
        let success_probability = 1.0 / 3.0;
        let score =
            calculate_score(&objective_value, &gas_cost, success_probability).to_f64_lossy();
        assert_eq!(score, 4999999999999999.);
    }
}
