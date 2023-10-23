use {
    super::settlement,
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
        Err(ScoringError::ObjectiveValueNonPositive(objective_value)) => {
            Err(score::Error::ObjectiveValueNonPositive(
                settlement::Error::ObjectiveValueNonPositive(objective_value.into()),
            ))
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
