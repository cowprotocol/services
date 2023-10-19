use {crate::domain::competition, solver::settlement_rater::ScoringError};
use {
    crate::{
        domain::{self, competition::score, eth},
        util::conv::u256::U256Ext,
    },
    //anyhow::Result,
    solver::settlement_rater::ScoreCalculator,
};

type SuccessProbability = f64;

#[derive(Debug, Clone)]
pub struct Score {
    pub inner: ScoreCalculator,
}

impl Score {
    pub fn new(score_cap: eth::U256, revert_protection: &domain::RevertProtection) -> Self {
        Self {
            inner: ScoreCalculator::new(
                score_cap.to_big_rational(),
                matches!(revert_protection, domain::RevertProtection::Disabled),
            ),
        }
    }

    pub fn score(
        &self,
        objective_value: eth::U256,
        gas: eth::Gas,
        gas_price: eth::GasPrice,
        success_probability: SuccessProbability,
    ) -> Result<competition::Score, score::Error> {
        let gas = gas.0.to_big_rational();
        let gas_price = eth::U256::from(gas_price.effective()).to_big_rational();
        let gas_cost = gas * gas_price;

        match self.inner.compute_score(
            &objective_value.to_big_rational(),
            &gas_cost,
            success_probability,
        ) {
            Ok(score) => Ok(score.into()),
            Err(ScoringError::ObjectiveValueNonPositive(_)) => {
                Err(score::Error::ObjectiveValueNonPositive)
            }
            Err(ScoringError::ScoreHigherThanObjective(_)) => {
                Err(score::Error::ScoreHigherThanObjective)
            }
            Err(ScoringError::SuccessProbabilityOutOfRange(_)) => Err(score::Error::Boundary(
                anyhow::anyhow!("unreachable, should have been checked by solvers"),
            )),
            Err(ScoringError::InternalError(err)) => Err(score::Error::Boundary(err)),
        }
    }
}
