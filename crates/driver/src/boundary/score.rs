use {
    crate::{
        domain::{
            competition::score::{self, SuccessProbability},
            eth,
        },
        util::conv::u256::U256Ext,
    },
    score::{ObjectiveValue, Score},
    solver::settlement_rater::{ScoreCalculator, ScoringError},
};

pub fn score(
    score_cap: Score,
    objective_value: ObjectiveValue,
    success_probability: SuccessProbability,
    failure_cost: eth::GasCost,
) -> Result<Score, score::Error> {
    match ScoreCalculator::new(score_cap.0.to_big_rational()).compute_score(
        &objective_value.0.to_big_rational(),
        failure_cost.0 .0.to_big_rational(),
        success_probability.0,
    ) {
        Ok(score) => Ok(score.into()),
        Err(ScoringError::ObjectiveValueNonPositive(_)) => {
            Err(score::Error::ObjectiveValueNonPositive)
        }
        Err(ScoringError::SuccessProbabilityOutOfRange(value)) => Err(
            score::Error::SuccessProbabilityOutOfRange(SuccessProbability(value)),
        ),
        Err(ScoringError::InternalError(err)) => Err(score::Error::Boundary(err)),
    }
}
