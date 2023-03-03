use {
    super::SettlementSimulating,
    crate::{
        objective_value::Inputs,
        settlement::Settlement,
        solver::score_computation::ScoreCalculator,
    },
    gas_estimation::GasPrice1559,
    num::{BigRational, FromPrimitive},
    shared::{external_prices::ExternalPrices, http_solver::model::Score},
};

pub async fn optimize_score(
    settlement: Settlement,
    settlement_simulator: &impl SettlementSimulating,
    score_calculator: &ScoreCalculator,
    gas_price: GasPrice1559,
    prices: &ExternalPrices,
) -> Settlement {
    let gas_amount = match settlement_simulator
        .settlement_would_succeed(settlement.clone())
        .await
    {
        Ok(gas_amount) => gas_amount, // todo multiply with 0.9 for more real value
        Err(_) => return settlement,
    };

    let inputs = Inputs::from_settlement(
        &settlement,
        prices,
        BigRational::from_f64(gas_price.effective_gas_price()).unwrap(),
        &gas_amount,
    );

    let score = score_calculator
        .compute_score(inputs, settlement.trades().count())
        .map(Score::Score);

    Settlement {
        score: settlement.score.or(score), // overwrite score if it was not set before
        ..settlement
    }
}
