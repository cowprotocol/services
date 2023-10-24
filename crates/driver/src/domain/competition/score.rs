use {
    crate::{boundary, domain::eth},
    std::cmp::Ordering,
};

impl Score {
    pub fn new(
        score_cap: Score,
        objective_value: ObjectiveValue,
        success_probability: SuccessProbability,
        failure_cost: eth::Ether,
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

impl std::ops::Add for Score {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl num::Zero for Score {
    fn zero() -> Self {
        Self(eth::U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
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

/// Represents the objective value of a solution. This is not an artifical value
/// like score. This is a real value that solution provides and it's based on
/// the surplus and fees of the solution.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct ObjectiveValue(pub eth::U256);

impl From<eth::U256> for ObjectiveValue {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// Comparing scores and objective values is needed to make sure the score is
/// not higher than the objective value, which is a requirement for the score to
/// be valid.
impl std::cmp::PartialEq<ObjectiveValue> for Score {
    fn eq(&self, other: &ObjectiveValue) -> bool {
        self.0.eq(&other.0)
    }
}

impl std::cmp::PartialOrd<ObjectiveValue> for Score {
    fn partial_cmp(&self, other: &ObjectiveValue) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("objective value is non-positive")]
    ObjectiveValueNonPositive,
    #[error("score is zero")]
    ZeroScore,
    #[error("objective value {0:?} is higher than the objective {1:?}")]
    ScoreHigherThanObjective(Score, ObjectiveValue),
    #[error("success probability is out of range {0:?}")]
    SuccessProbabilityOutOfRange(SuccessProbability),
    #[error("invalid objective value")]
    Boundary(#[from] boundary::Error),
}
