use crate::{
    boundary,
    domain::{eth, mempools},
};

impl Score {
    pub fn new(
        score_cap: eth::U256,
        revert_protection: &mempools::RevertProtection,
        objective_value: eth::U256,
        gas: eth::Gas,
        gas_price: eth::GasPrice,
        success_probability: SuccessProbability,
    ) -> Result<Self, Error> {
        let boundary = boundary::ScoreCalculator::new(score_cap, revert_protection);
        boundary.score(objective_value, gas, gas_price, success_probability)
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
