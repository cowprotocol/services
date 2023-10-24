use {
    crate::{
        domain::{
            competition::{
                self,
                score::{self, ObjectiveValue, SuccessProbability},
            },
            eth,
        },
        util::conv::u256::U256Ext,
    },
    score::Score,
    solver::settlement_rater::{ScoreCalculator, ScoringError},
};

pub fn score(
    score_cap: Score,
    objective_value: ObjectiveValue,
    success_probability: SuccessProbability,
    failure_cost: eth::Ether,
) -> Result<competition::Score, score::Error> {
    match ScoreCalculator::new(score_cap.0.to_big_rational()).compute_score(
        &objective_value.0.to_big_rational(),
        failure_cost.0.to_big_rational(),
        success_probability.0,
    ) {
        Ok(score) => Ok(score.into()),
        Err(ScoringError::ObjectiveValueNonPositive(_)) => {
            Err(score::Error::ObjectiveValueNonPositive)
        }
        Err(ScoringError::ScoreHigherThanObjective(score)) => {
            Err(score::Error::ScoreHigherThanObjective(
                eth::U256::from_big_rational(&score)
                    .unwrap_or_default()
                    .into(),
            ))
        }
        Err(ScoringError::SuccessProbabilityOutOfRange(value)) => Err(
            score::Error::SuccessProbabilityOutOfRange(SuccessProbability(value)),
        ),
        Err(ScoringError::InternalError(err)) => Err(score::Error::Boundary(err)),
    }
}
