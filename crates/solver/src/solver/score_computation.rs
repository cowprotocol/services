// Suppose a solver generates a solution, and we compute the three variables:
// surplus, fees and gas_cost. Currently, the objective is then simply equal to
// surplus + fees - gas_cost. However, for CIP-20, we now have one additional
// step for each solver. So for each solver we have trained a revert risk model,
// which essentially is a function that computes the probability of the
// settlement not reverting, and is a function of 3 parameters.
// p (num_orders_in_sol, gas_used, gas_price).
// So we compute this p, and then the score is equal to:
// score = p * (surplus + fees) - gas_cost

use {crate::objective_value, ethcontract::U256, num::ToPrimitive, std::ops::Neg};

#[derive(Debug, Default)]
pub struct ScoreCalculator {
    a: f64,
    b: f64,
    c: f64,
    x: f64,
}

impl ScoreCalculator {
    pub fn new(a: f64, b: f64, c: f64, x: f64) -> Self {
        Self { a, b, c, x }
    }

    pub fn compute_score(
        &self,
        inputs: objective_value::Inputs,
        nmb_orders: usize,
    ) -> Option<U256> {
        let surplus = inputs.surplus_given.to_f64()?;
        let fees = inputs.solver_fees.to_f64()?;
        let gas_amount = inputs.gas_amount.to_f64()?;
        let gas_price = inputs.gas_price.to_f64()?;

        let exponent =
            self.x.neg() - self.a * gas_amount - self.b * gas_price - self.c * nmb_orders as f64;
        let revert_probability = 1. / (1. + exponent.exp());
        let score = revert_probability * (surplus + fees) - gas_amount * gas_price;
        Some(U256::from_f64_lossy(score))
    }
}

pub struct Arguments {
    // todo
}
