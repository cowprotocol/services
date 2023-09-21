use crate::{boundary, domain::eth, infra::mempool};

#[derive(Debug, Clone)]
pub struct ScoreCalculator {
    boundary: boundary::score_calculator::ScoreCalculator,
}

impl ScoreCalculator {
    pub fn new(score_cap: eth::U256, mempools: Vec<mempool::Config>) -> Self {
        Self {
            boundary: boundary::score_calculator::ScoreCalculator::new(score_cap, mempools),
        }
    }

    pub fn score(
        &self,
        objective_value: &eth::U256,
        gas_cost: &eth::Ether,
        success_probability: f64,
    ) -> anyhow::Result<eth::U256> {
        self.boundary
            .score(objective_value, gas_cost, success_probability)
    }
}
