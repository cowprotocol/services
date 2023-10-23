use {
    crate::{boundary, domain::eth},
    num::BigRational,
};

impl Score {
    pub fn new(
        score_cap: eth::U256,
        objective_value: eth::U256,
        success_probability: SuccessProbability,
        failure_cost: eth::U256,
    ) -> Result<Self, Error> {
        boundary::score::score(
            score_cap,
            objective_value,
            success_probability,
            failure_cost,
        )
    }
}

/// Represents a single value suitable for comparing/ranking solutions.
/// This is a final score that is observed by the autopilot.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Score(pub eth::U256);

impl From<Score> for eth::U256 {
    fn from(value: Score) -> Self {
        value.0
    }
}

impl From<eth::U256> for Score {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// Represents the probability that a solution will be successfully settled.
#[derive(Debug, Copy, Clone)]
pub struct SuccessProbability(pub f64);

impl From<f64> for SuccessProbability {
    fn from(value: f64) -> Self {
        Self(value)
    }
}

#[derive(Debug, Clone)]
pub struct ObjectiveValue(pub BigRational);

impl From<BigRational> for ObjectiveValue {
    fn from(value: BigRational) -> Self {
        Self(value)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("objective value is non-positive")]
    ObjectiveValueNonPositive(ObjectiveValue),
    #[error("objective value is higher than the objective")]
    ScoreHigherThanObjective(Score),
    #[error("success probability is out of range {0:?}")]
    SuccessProbabilityOutOfRange(SuccessProbability),
    #[error("invalid objective value")]
    Boundary(#[from] boundary::Error),
}
