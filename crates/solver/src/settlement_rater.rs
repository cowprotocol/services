use {
    anyhow::{anyhow, Context, Result},
    num::{zero, BigRational, CheckedDiv, One},
    number::conversions::big_rational_to_u256,
    primitive_types::U256,
    std::cmp::min,
};

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
