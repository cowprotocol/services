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
        solver::{Simulation, SimulationError, SimulationWithError, SolverInfo},
    },
    anyhow::{anyhow, Context, Result},
    contracts::GPv2Settlement,
    ethcontract::Account,
    gas_estimation::GasPrice1559,
    model::solver_competition::Score,
    num::{zero, BigRational, CheckedDiv, One},
    number::conversions::big_rational_to_u256,
    primitive_types::U256,
    shared::{
        code_fetching::CodeFetching,
        ethrpc::Web3,
        external_prices::ExternalPrices,
        http_solver::{
            self,
            model::{InternalizationStrategy, SimulatedTransaction},
        },
    },
    std::{borrow::Borrow, cmp::min, sync::Arc},
    web3::types::AccessList,
};

type GasEstimate = U256;

pub enum SimulateError {
    FailedSimulation(SimulationWithError),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for SimulateError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }
}

#[derive(Debug)]
pub enum RatingError {
    FailedSimulation(SimulationWithError),
    FailedScoring(ScoringError),
    Internal(anyhow::Error),
}

impl From<SimulateError> for RatingError {
    fn from(error: SimulateError) -> Self {
        match error {
            SimulateError::FailedSimulation(failure) => Self::FailedSimulation(failure),
            SimulateError::Internal(error) => Self::Internal(error),
        }
    }
}

impl From<ScoringError> for RatingError {
    fn from(error: ScoringError) -> Self {
        Self::FailedScoring(error)
    }
}
impl From<anyhow::Error> for RatingError {
    fn from(error: anyhow::Error) -> Self {
        Self::Internal(error)
    }
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
    ) -> Result<RatedSettlement, RatingError>;
}

pub struct SettlementRater {
    pub access_list_estimator: Arc<dyn AccessListEstimating>,
    pub code_fetcher: Arc<dyn CodeFetching>,
    pub settlement_contract: GPv2Settlement,
    pub web3: Web3,
    pub score_calculator: ScoreCalculator,
    pub consider_cost_failure: bool,
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
    ) -> Result<(Simulation, GasEstimate), SimulateError> {
        let access_list = self
            .generate_access_list(&solver.account, settlement, gas_price, internalization)
            .await;
        let block_number = self
            .web3
            .eth()
            .block_number()
            .await
            .context("failed to get block number")?
            .as_u64();
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

        match simulation_result {
            Ok(gas_estimate) => Ok((simulation, gas_estimate)),
            Err(error) => Err(SimulateError::FailedSimulation(SimulationWithError {
                simulation,
                error: error.into(),
            })),
        }
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
    ) -> Result<RatedSettlement, RatingError> {
        // first simulate settlements without internalizations to make sure they pass
        let _ = self
            .simulate_settlement(
                solver,
                &settlement,
                gas_price,
                InternalizationStrategy::EncodeAllInteractions,
            )
            .await?;

        // since rating is done with internalizations, repeat the simulations for
        // previously succeeded simulations
        let (simulation, gas_estimate) = self
            .simulate_settlement(
                solver,
                &settlement,
                gas_price,
                InternalizationStrategy::SkipInternalizableInteraction,
            )
            .await?;

        let effective_gas_price =
            BigRational::from_float(gas_price.effective_gas_price()).expect("Invalid gas price.");

        let solver_balance = self
            .web3
            .eth()
            .balance(solver.account.address(), None)
            .await
            .unwrap_or_default();

        let gas_limit = gas_limit_for_estimate(gas_estimate);
        let required_balance =
            gas_limit.saturating_mul(U256::from_f64_lossy(gas_price.max_fee_per_gas));

        if solver_balance < required_balance {
            return Err(RatingError::FailedSimulation(SimulationWithError {
                simulation,
                error: SimulationError::InsufficientBalance {
                    needs: required_balance,
                    has: solver_balance,
                },
            }));
        }

        let earned_fees = settlement.total_earned_fees(prices);
        let inputs = {
            let gas_amount = match settlement.score {
                http_solver::model::Score::RiskAdjusted { gas_amount, .. } => {
                    gas_amount.unwrap_or(gas_estimate)
                }
                _ => gas_estimate,
            };
            crate::objective_value::Inputs::from_settlement(
                &settlement,
                prices,
                effective_gas_price.clone(),
                &gas_amount,
            )
        };

        let objective_value = inputs.objective_value();
        let score = match settlement.score {
            http_solver::model::Score::Solver { score } => Score::Solver(score),
            http_solver::model::Score::RiskAdjusted {
                success_probability,
                ..
            } => {
                let cost_fail = self
                    .consider_cost_failure
                    .then(|| inputs.gas_cost())
                    .unwrap_or_else(zero);
                Score::ProtocolWithSolverRisk(self.score_calculator.compute_score(
                    &objective_value,
                    cost_fail,
                    success_probability,
                )?)
            }
            http_solver::model::Score::Surplus => {
                unimplemented!("SettlementRater is not used anymore")
            }
        };

        let rated_settlement = RatedSettlement {
            id,
            settlement,
            surplus: inputs.surplus_given,
            earned_fees,
            solver_fees: inputs.solver_fees,
            // save simulation gas estimate even if the solver provided gas amount
            // it's safer and more accurate since simulation gas estimate includes pre/post hooks
            gas_estimate,
            gas_price: effective_gas_price,
            objective_value,
            score,
            ranking: Default::default(),
        };
        Ok(rated_settlement)
    }
}

#[derive(Debug)]
pub enum ScoringError {
    ObjectiveValueNonPositive(BigRational),
    SuccessProbabilityOutOfRange(f64),
    InternalError(anyhow::Error),
}

impl From<anyhow::Error> for ScoringError {
    fn from(error: anyhow::Error) -> Self {
        Self::InternalError(error)
    }
}

impl From<ScoringError> for anyhow::Error {
    fn from(error: ScoringError) -> Self {
        match error {
            ScoringError::InternalError(error) => error,
            ScoringError::ObjectiveValueNonPositive(objective) => {
                anyhow!("Objective value non-positive {}", objective)
            }
            ScoringError::SuccessProbabilityOutOfRange(success_probability) => {
                anyhow!("Success probability out of range {}", success_probability)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScoreCalculator {
    score_cap: BigRational,
}

impl ScoreCalculator {
    pub fn new(score_cap: BigRational) -> Self {
        Self { score_cap }
    }

    #[allow(clippy::result_large_err)]
    pub fn compute_score(
        &self,
        objective_value: &BigRational,
        cost_fail: BigRational,
        success_probability: f64,
    ) -> Result<U256, ScoringError> {
        if objective_value <= &zero() {
            return Err(ScoringError::ObjectiveValueNonPositive(
                objective_value.clone(),
            ));
        }
        if !(0.0..=1.0).contains(&success_probability) {
            return Err(ScoringError::SuccessProbabilityOutOfRange(
                success_probability,
            ));
        }

        let success_probability = BigRational::from_float(success_probability).unwrap();
        let optimal_score = compute_optimal_score(
            objective_value.clone(),
            success_probability.clone(),
            cost_fail.clone(),
            self.score_cap.clone(),
        )?;
        if optimal_score > *objective_value {
            tracing::error!(%optimal_score, %objective_value, %success_probability, %cost_fail,
                "Sanity check failed, score higher than objective, should never happen unless \
                 there's a bug in a computation"
            );
            return Err(anyhow!(
                "Sanity check failed, score higher than objective, should never happen unless \
                 there's a bug in a computation"
            )
            .into());
        }
        let score = big_rational_to_u256(&optimal_score).context("Bad conversion")?;
        Ok(score)
    }
}

fn compute_optimal_score(
    objective: BigRational,
    probability_success: BigRational,
    cost_fail: BigRational,
    score_cap: BigRational,
) -> Result<BigRational> {
    tracing::trace!(
        ?objective,
        ?probability_success,
        ?cost_fail,
        "Computing optimal score"
    );
    let probability_fail = BigRational::one() - probability_success.clone();

    // Computes the solvers' payoff (positive or negative) given the second
    // highest score. The optimal bidding is such that in the worst case
    // (reference score == winning score) the winning solver still breaks even
    // (profit(winning_score) = 0)
    let profit = |score_reference: BigRational| {
        profit(
            score_reference,
            objective.clone(),
            probability_success.clone(),
            cost_fail.clone(),
            score_cap.clone(),
        )
    };

    // The profit function is monotonically decreasing (a higher reference score
    // means smaller payout for the winner). In order to compute the zero value,
    // we need to evaluate the payoff function at two important points (`cap` and
    // `objective - cap`).
    // To see why, we draw the two additive ingredients of the
    // profit function separately (reward in the success case, penalty in the
    // failure case):
    //
    // Rewards                             Penalties
    // ▲                                   ▲
    // │____◄───profit(obj-cap)            ┌───────────────►
    // │    \                              │\            ref_score
    // │     \                             │ \
    // └──────\───────────►                │  \
    //         \  ref_score                    \__________
    //                          profit(cap)───►
    //
    // Success rewards are capped from above and thus constant until the reference
    // score reaches the `obj - cap`, then the profits shrink.
    // Failure penalties are capped from below and thus fall from zero until they
    // reach `cap`, then constant.
    // It's now important to know if we have already passed the zero point of the
    // payoff function at these two points as they influence the shape of the
    // curve.
    let profit_cap = profit(score_cap.clone());
    let profit_obj_minus_cap = profit(objective.clone() - score_cap.clone());
    tracing::trace!(
        ?profit_obj_minus_cap,
        ?profit_cap,
        "profit score minus cap and profit cap"
    );

    let score = {
        if profit_obj_minus_cap >= zero() && profit_cap <= zero() {
            // The optimal score lies between `objective - cap` and `cap`. The cap is
            // therefore not affecting the optimal score in any way:
            //
            // `payoff(score) = probability_success * (objective - score) - probability_fail
            // * (score + cost_fail)`
            //
            // Solving for score when `payoff(score) = 0`, we get
            let score = probability_success * objective.clone() - probability_fail * cost_fail;
            Ok(score)
        } else if profit_obj_minus_cap >= zero() && profit_cap > zero() {
            // Optimal score is larger than `objective - cap` and `cap`. The penalty cap is
            // therefore affecting payoff in case of reverts:
            //
            // payoff(score) = probability_success * (objective - score) - probability_fail
            // * (cap + cost_fail)
            //
            // Solving for score when `payoff(score) = 0`, we get
            let score = objective.clone()
                - probability_fail
                    .checked_div(&probability_success)
                    .context("division by success")?
                    * (score_cap + cost_fail);
            Ok(score)
        } else if profit_obj_minus_cap < zero() && profit_cap <= zero() {
            // Optimal score is smaller than `objective - cap` and `cap`. The reward cap is
            // therefore affecting payoff in case of success:
            //
            // payoff(score) = probability_success * cap - probability_fail * (score +
            // cost_fail)
            //
            // Solving for score when `payoff(score) = 0`, we get
            let score = probability_success
                .checked_div(&probability_fail)
                .context("division by fail")?
                * score_cap
                - cost_fail;
            Ok(score)
        } else {
            // The optimal score would lie between `cap` and `objective - cap`.
            // This implies that cap is binding from below and above. The payoff would have
            // to be constant in the reference score, which is a
            // contradiction to having a zero.
            Err(anyhow!("Unreachable branch in optimal score computation"))
        }
    };

    tracing::trace!(?score, "Optimal score");
    score
}

fn profit(
    score_reference: BigRational,
    objective: BigRational,
    probability_success: BigRational,
    cost_fail: BigRational,
    score_cap: BigRational,
) -> BigRational {
    tracing::trace!(
        ?score_reference,
        ?objective,
        ?probability_success,
        ?cost_fail,
        ?score_cap,
        "Computing profit"
    );

    // this much is payed out to the solver if the transaction succeeds
    let reward = min(objective - score_reference.clone(), score_cap.clone());
    // this much is solver penalized if the transaction fails
    let penalty = min(score_reference, score_cap) + cost_fail;

    tracing::trace!(?reward, ?penalty, "Reward and penalty");

    // final profit is the combination of the two above
    let probability_fail = BigRational::one() - probability_success.clone();
    let profit = probability_success * reward - probability_fail * penalty;

    tracing::trace!(?profit, "Final profit");
    profit
}

#[cfg(test)]
mod tests {
    use {
        num::{BigRational, Zero},
        primitive_types::U256,
        shared::conversions::U256Ext,
    };

    fn calculate_score(objective_value: &BigRational, success_probability: f64) -> U256 {
        let score_cap = BigRational::from_float(1e16).unwrap();
        let score_calculator = super::ScoreCalculator::new(score_cap);
        score_calculator
            .compute_score(objective_value, BigRational::zero(), success_probability)
            .unwrap()
    }

    #[test]
    fn compute_score_with_success_probability_case_1() {
        // testing case `payout_score_minus_cap >= zero() && payout_cap <= zero()`
        let objective_value = num::BigRational::from_float(1e16).unwrap();
        let success_probability = 0.9;
        let score = calculate_score(&objective_value, success_probability).to_f64_lossy();
        assert_eq!(score, 9e15);
    }

    #[test]
    fn compute_score_with_success_probability_case_2() {
        // testing case `payout_score_minus_cap >= zero() && payout_cap > zero()`
        let objective_value = num::BigRational::from_float(1e17).unwrap();
        let success_probability = 2.0 / 3.0;
        let score = calculate_score(&objective_value, success_probability).to_f64_lossy();
        assert_eq!(score, 94999999999999999.);
    }

    #[test]
    fn compute_score_with_success_probability_case_3() {
        // testing case `payout_score_minus_cap < zero() && payout_cap <= zero()`
        let objective_value = num::BigRational::from_float(1e17).unwrap();
        let success_probability = 1.0 / 3.0;
        let score = calculate_score(&objective_value, success_probability).to_f64_lossy();
        assert_eq!(score, 4999999999999999.);
    }

    #[test]
    fn compute_score_with_success_probability_one() {
        // if success_probability is 1.0, the score should be equal to the objective
        // value
        let objective_value = num::BigRational::from_float(1e16).unwrap();
        let success_probability = 1.;
        let score = calculate_score(&objective_value, success_probability);
        assert_eq!(score.to_big_rational(), objective_value);
    }
}
