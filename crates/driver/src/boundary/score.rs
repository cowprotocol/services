use {
    crate::{
        boundary,
        domain::{
            competition::score::{self, risk::SuccessProbability},
            eth,
        },
        util::conv::u256::U256Ext,
    },
    score::{ObjectiveValue, Score},
    solver::settlement_rater::ScoreCalculator,
};

pub fn score(
    score_cap: Score,
    objective_value: ObjectiveValue,
    success_probability: SuccessProbability,
    failure_cost: eth::GasCost,
) -> Result<Score, boundary::Error> {
    match ScoreCalculator::new(score_cap.0.to_big_rational()).compute_score(
        &objective_value.0.get().to_big_rational(),
        failure_cost.0 .0.to_big_rational(),
        success_probability.0,
    ) {
        Ok(score) => Ok(score.into()),
        Err(err) => Err(err.into()),
    }
}
