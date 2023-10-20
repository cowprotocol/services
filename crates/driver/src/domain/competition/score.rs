use crate::{boundary, domain::eth};

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
#[derive(Debug, Clone)]
pub struct SuccessProbability(pub f64);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("objective value is non-positive")]
    ObjectiveValueNonPositive,
    #[error("objective value is higher than the objective")]
    ScoreHigherThanObjective,
    #[error("invalid objective value")]
    Boundary(#[from] boundary::Error),
}
