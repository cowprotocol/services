use {
    crate::{
        domain::{
            competition::{
                self,
                score::{self, SuccessProbability},
            },
            eth,
        },
        util::conv::u256::U256Ext,
    },
    solver::settlement_rater::{ScoreCalculator, ScoringError},
};

pub fn score(
    score_cap: eth::U256,
    objective_value: eth::U256,
    success_probability: SuccessProbability,
    failure_cost: eth::U256,
) -> Result<competition::Score, score::Error> {
    match ScoreCalculator::new(score_cap.to_big_rational()).compute_score(
        &objective_value.to_big_rational(),
        failure_cost.to_big_rational(),
        success_probability.0,
    ) {
        Ok(score) => Ok(score.into()),
        Err(ScoringError::ObjectiveValueNonPositive(_)) => {
            Err(score::Error::ObjectiveValueNonPositive)
        }
        Err(ScoringError::ScoreHigherThanObjective(_)) => {
            Err(score::Error::ScoreHigherThanObjective)
        }
        Err(ScoringError::SuccessProbabilityOutOfRange(_)) => Err(score::Error::Boundary(
            anyhow::anyhow!("unreachable, should have been checked by solvers"),
        )),
        Err(ScoringError::InternalError(err)) => Err(score::Error::Boundary(err)),
    }
}
