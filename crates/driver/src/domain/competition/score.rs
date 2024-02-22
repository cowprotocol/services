use {
    super::solution,
    crate::{boundary, domain::eth},
    std::cmp::Ordering,
};

/// Represents a single value suitable for comparing/ranking solutions.
/// This is a final score that is observed by the autopilot.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Score(pub eth::NonZeroU256);

impl From<Score> for eth::NonZeroU256 {
    fn from(value: Score) -> Self {
        value.0
    }
}

impl TryFrom<eth::U256> for Score {
    type Error = Error;

    fn try_from(value: eth::U256) -> Result<Self, Self::Error> {
        Ok(Self(eth::NonZeroU256::new(value).ok_or(Error::ZeroScore)?))
    }
}

/// Represents the observed quality of a solution. It's defined as surplus +
/// fees.
#[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
pub struct Quality(pub eth::U256);

impl From<eth::U256> for Quality {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

/// Comparing scores and observed quality is needed to make sure the score is
/// not higher than the observed quality, which is a requirement for the score
/// to be valid.
impl std::cmp::PartialEq<Quality> for Score {
    fn eq(&self, other: &Quality) -> bool {
        self.0.get().eq(&other.0)
    }
}

impl std::cmp::PartialOrd<Quality> for Score {
    fn partial_cmp(&self, other: &Quality) -> Option<Ordering> {
        self.0.get().partial_cmp(&other.0)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The solution has zero score. Zero score solutions are not allowed as per
    /// CIP20 definition. The main reason being that reference score is zero,
    /// and if only one solution is in competition with zero score, that
    /// solution would receive 0 reward (reward = score - reference score).
    #[error("score is zero")]
    ZeroScore,
    /// Protocol does not allow solutions that are claimed to be "better" than
    /// the actual value they bring (quality). It is expected that score
    /// is always lower than quality, because there is always some
    /// execution cost that needs to be incorporated into the score and lower
    /// it.
    #[error("score {0:?} is higher than the quality {1:?}")]
    ScoreHigherThanQuality(Score, Quality),
    /// Errors only applicable to scores that use success probability.
    #[error(transparent)]
    RiskAdjusted(#[from] risk::Error),
    #[error(transparent)]
    Boundary(#[from] boundary::Error),
    #[error(transparent)]
    SolutionError(#[from] solution::SolutionError),
}

pub mod risk {
    //! Contains functionality and error types for scores that are based on
    //! success probability.

    use {
        super::{Quality, Score},
        crate::{
            boundary,
            domain::{eth, eth::GasCost},
        },
    };

    impl Score {
        /// Constructs a score based on the success probability of a solution.
        pub fn new(
            score_cap: Score,
            objective_value: ObjectiveValue,
            success_probability: SuccessProbability,
            failure_cost: eth::GasCost,
        ) -> Result<Self, super::Error> {
            boundary::score::score(
                score_cap,
                objective_value,
                success_probability,
                failure_cost,
            )
        }
    }

    /// Represents the probability that a solution will be successfully settled.
    #[derive(Debug, Copy, Clone)]
    pub struct SuccessProbability(pub f64);

    impl TryFrom<f64> for SuccessProbability {
        type Error = Error;

        fn try_from(value: f64) -> Result<Self, Self::Error> {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::SuccessProbabilityOutOfRange(value));
            }
            Ok(Self(value))
        }
    }

    /// Represents the objective value of a solution. This is not an artifical
    /// value like score. This is a real value that solution provides and
    /// it's defined as Quality - GasCost.
    #[derive(Debug, Default, PartialEq, Eq, PartialOrd, Ord, Copy, Clone)]
    pub struct ObjectiveValue(pub eth::NonZeroU256);

    /// Substitution for constructor of ObjectiveValue
    /// ObjectiveValue = Quality - GasCost
    impl std::ops::Sub<GasCost> for Quality {
        type Output = Result<ObjectiveValue, Error>;

        fn sub(self, other: GasCost) -> Self::Output {
            if self.0 > other.get().0 {
                Ok(ObjectiveValue(
                    eth::NonZeroU256::new(self.0 - other.get().0).unwrap(),
                ))
            } else {
                Err(Error::ObjectiveValueNonPositive(self, other))
            }
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum Error {
        /// Solution has success probability that is outside of the allowed
        /// range [0, 1]
        #[error("success probability is out of range {0:?}")]
        SuccessProbabilityOutOfRange(f64),
        /// Objective value is defined as surplus + fees - gas costs. Protocol
        /// doesn't allow solutions that cost more than they bring to the users
        /// and protocol. Score calculator does not make sense for such
        /// solutions, since score calculator is expected to return
        /// value (0, ObjectiveValue]
        #[error("objective value is non-positive, quality {0:?}, gas cost {1:?}")]
        ObjectiveValueNonPositive(Quality, GasCost),
        #[error(transparent)]
        Boundary(#[from] boundary::Error),
    }
}
