use {
    super::mempool,
    crate::{
        domain::{eth, eth::Ether},
        util::conv::u256::U256Ext,
    },
    anyhow::Result,
    solver::arguments::TransactionStrategyArg,
};

#[derive(Debug, Clone)]
pub struct ScoreCalculator {
    inner: solver::settlement_rater::ScoreCalculator,
}

impl ScoreCalculator {
    pub fn new(score_cap: eth::U256, mempools: Vec<mempool::Config>) -> Self {
        Self {
            inner: solver::settlement_rater::ScoreCalculator::new(
                score_cap.to_big_rational(),
                mempools
                    .iter()
                    .map(|mempool| match mempool.kind {
                        mempool::Kind::Public(high_risk) => (
                            TransactionStrategyArg::PublicMempool,
                            matches!(high_risk, mempool::HighRisk::Disabled),
                        ),
                        mempool::Kind::Flashbots { .. } => {
                            (TransactionStrategyArg::Flashbots, false)
                        }
                    })
                    .collect(),
            ),
        }
    }

    pub fn score(
        &self,
        objective_value: &eth::U256,
        gas_cost: &Ether,
        success_probability: f64,
    ) -> Result<eth::U256> {
        self.inner
            .compute_score(
                &objective_value.to_big_rational(),
                &gas_cost.0.to_big_rational(),
                success_probability,
            )
            .map_err(Into::into)
    }
}
