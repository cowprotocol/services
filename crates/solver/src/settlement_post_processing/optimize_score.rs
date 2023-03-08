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

pub async fn compute_score(
    settlement: Settlement,
    settlement_simulator: &impl SettlementSimulating,
    score_calculator: &ScoreCalculator,
    gas_price: GasPrice1559,
    prices: &ExternalPrices,
) -> Settlement {
    if settlement.score.is_some() {
        return settlement;
    }

    let gas_amount = match settlement_simulator.estimate_gas(settlement.clone()).await {
        // Multiply by 0.9 to get more realistic gas amount.
        // This is because the gas estimation is not accurate enough and does not take the
        // EVM gas refund into account.
        Ok(gas_amount) => gas_amount * 9 / 10,
        Err(_) => return settlement,
    };

    let inputs = Inputs::from_settlement(
        &settlement,
        prices,
        BigRational::from_f64(gas_price.effective_gas_price()).unwrap(),
        &gas_amount,
    );
    let nmb_orders = settlement.trades().count();

    let score = score_calculator
        .calculate(inputs, nmb_orders)
        .map(Score::Score);

    Settlement {
        score,
        ..settlement
    }
}
