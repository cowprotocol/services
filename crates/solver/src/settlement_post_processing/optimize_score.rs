use {
    super::SettlementSimulating,
    crate::{settlement::Settlement, solver::risk_computation::RiskCalculator},
    anyhow::Result,
    ethcontract::Address,
    gas_estimation::GasPrice1559,
};

pub async fn compute_success_probability(
    settlement: &Settlement,
    settlement_simulator: &impl SettlementSimulating,
    risk_calculator: &RiskCalculator,
    gas_price: GasPrice1559,
    solver: &Address,
) -> Result<f64> {
    let gas_amount = settlement_simulator
        .estimate_gas(settlement.clone())
        .await
        .map(|gas_amount| {
            // Multiply by 0.9 to get more realistic gas amount.
            // This is because the gas estimation is not accurate enough and does not take
            // the EVM gas refund into account.
            gas_amount.to_f64_lossy() * 0.9
        })?;
    let gas_price = gas_price.effective_gas_price();
    let nmb_orders = settlement.trades().count();

    let success_probability = risk_calculator.calculate(gas_amount, gas_price, nmb_orders)?;

    tracing::debug!(
        ?solver,
        ?success_probability,
        "computed success_probability",
    );

    Ok(success_probability)
}
