use {
    crate::{
        boundary,
        domain::{
            competition::score::{self, SuccessProbability},
            eth,
        },
        util::conv::u256::U256Ext,
    },
    bigdecimal::Zero,
    score::{ObjectiveValue, Score},
    solver::settlement_rater::ScoreCalculator,
};

pub fn score(
    score_cap: Score,
    objective_value: ObjectiveValue,
    success_probability: SuccessProbability,
    failure_cost: eth::GasCost,
) -> Result<Score, Error> {
    if objective_value.is_zero() {
        return Err(Error::ObjectiveValueNonPositive);
    }
    if !(0.0..=1.0).contains(&success_probability.0) {
        return Err(Error::SuccessProbabilityOutOfRange(success_probability));
    }

    match ScoreCalculator::new(score_cap.0.to_big_rational()).compute_score(
        &objective_value.0.to_big_rational(),
        failure_cost.0 .0.to_big_rational(),
        success_probability.0,
    ) {
        Ok(score) => Ok(score.into()),
        Err(err) => Err(Error::Boundary(err.into())),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Solution has success probability that is outside of the allowed range
    /// [0, 1]
    #[error("success probability is out of range {0:?}")]
    SuccessProbabilityOutOfRange(SuccessProbability),
    /// Objective value is defined as surplus + fees - gas costs. Protocol
    /// doesn't allow solutions that cost more than they bring to the users and
    /// protocol. Score calculator does not make sense for such solutions, since
    /// score calculator is expected to return value (0, ObjectiveValue]
    #[error("objective value is non-positive")]
    ObjectiveValueNonPositive,
    #[error(transparent)]
    Boundary(#[from] boundary::Error),
}
