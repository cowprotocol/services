use {
    crate::{
        boundary,
        domain::{eth, eth::GasCost},
    },
    std::cmp::Ordering,
};

impl Score {
    pub fn new(
        score_cap: Score,
        objective_value: ObjectiveValue,
        success_probability: SuccessProbability,
        failure_cost: eth::GasCost,
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

/// Represents the observed quality of a solution. This is not an artifical
/// value like score. This is a real value that solution provides and it's
/// defined as surplus + fees.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Quality(pub eth::U256);

impl From<eth::U256> for Quality {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// ObjectiveValue = Quality - GasCost
impl std::ops::Sub<GasCost> for Quality {
    type Output = ObjectiveValue;

    fn sub(self, other: GasCost) -> Self::Output {
        ObjectiveValue(self.0.saturating_sub(other.0 .0))
    }
}

/// Comparing scores and observed quality is needed to make sure the score is
/// not higher than the observed quality, which is a requirement for the score
/// to be valid.
impl std::cmp::PartialEq<Quality> for Score {
    fn eq(&self, other: &Quality) -> bool {
        self.0.eq(&other.0)
    }
}

impl std::cmp::PartialOrd<Quality> for Score {
    fn partial_cmp(&self, other: &Quality) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

/// Represents the objective value of a solution. This is not an artifical value
/// like score. This is a real value that solution provides and it's based on
/// formula observed quality - gas costs.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct ObjectiveValue(pub eth::U256);

impl From<eth::U256> for ObjectiveValue {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

impl std::ops::Add for ObjectiveValue {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

impl num::Zero for ObjectiveValue {
    fn zero() -> Self {
        Self(eth::U256::zero())
    }

    fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("score is zero")]
    ZeroScore,
    #[error("score {0:?} is higher than the quality {1:?}")]
    ScoreHigherThanQuality(Score, Quality),
    #[error("success probability is out of range {0:?}")]
    /// [ONLY APPLICABLE TO SUCCESS PROBABILITY SCORES]
    SuccessProbabilityOutOfRange(SuccessProbability),
    #[error("objective value is non-positive")]
    /// [ONLY APPLICABLE TO SUCCESS PROBABILITY SCORES]
    ObjectiveValueNonPositive,
    #[error(transparent)]
    Boundary(#[from] boundary::Error),
}
