// Suppose a solver generates a solution, and we compute the three variables:
// surplus, fees and gas_cost. Currently, the objective is then simply equal to
// surplus + fees - gas_cost. However, for CIP-20, we now have one additional
// step for each solver. So for each solver we have trained a revert risk model,
// which essentially is a function that computes the probability of the
// settlement not reverting, and is a function of 3 parameters.
// p (num_orders_in_sol, gas_used, gas_price).
// So we compute this p, and then the score is equal to:
// score = p * (surplus + fees) - gas_cost

use {
    super::SolverType,
    crate::objective_value,
    anyhow::{Context, Result},
    clap::{Parser, ValueEnum},
    ethcontract::U256,
    num::ToPrimitive,
    std::{
        collections::HashMap,
        fmt::{Display, Formatter},
        ops::Neg,
        str::FromStr,
    },
};

#[derive(Debug, Default, Clone)]
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

const DEFAULT_SCORE_PARAMETERS: &str =
    "\
    Naive,0.5604082285267333,0.00285114179288399,0.06499875450001853,3.3987949311136787;Baseline,\
     -0.24391894879979226,-0.05809501139187965,-0.000013222507455295696,4.27946195371547;CowDexAg,\
     -0.9613998308805674,-0.14435150204689684,0.13923418474574772,2.7565258390467178;OneInch,-0.\
     32674185936325467,-0.05930446215554123,-0.33031769043234466,3.144609301500272;Paraswap,-0.\
     7815504846264341,-0.06965336115721313,0.0701725936991023,3.2617622830143453;ZeroEx,-1.\
     399997494341399,-0.04522233479453635,0.11066085229796373,2.7150950015915676;BalancerSor,-0.\
     7070951919365344,-0.1841886790519467,0.34189609422313544,3.6849833670945027";

#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    #[clap(long, env, default_value = DEFAULT_SCORE_PARAMETERS)]
    score_parameters: ScoreParameters,
}

impl Arguments {
    pub fn get_calculator(&self, solver: SolverType) -> Option<ScoreCalculator> {
        self.score_parameters.0.get(&solver).cloned()
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "score_parameters: {:?}", self.score_parameters)
    }
}

#[derive(Debug, Clone)]
pub struct ScoreParameters(HashMap<SolverType, ScoreCalculator>);

// expected string format:
// "Naive,2.3,3,4.5,7;Baseline,1.2,3.4,5.6,8"
impl FromStr for ScoreParameters {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let score_parameters = s
            .split(';')
            .map(|part| {
                let (solver, parameters) = part
                    .split_once(',')
                    .context("malformed solver score parameters")?;
                let mut parameters = parameters.split(',');
                let a = parameters
                    .next()
                    .context("missing a parameter for score")?
                    .parse()?;
                let b = parameters
                    .next()
                    .context("missing b parameter for score")?
                    .parse()?;
                let c = parameters
                    .next()
                    .context("missing c parameter for score")?
                    .parse()?;
                let x = parameters
                    .next()
                    .context("missing x parameter for score")?
                    .parse()?;
                Ok((
                    SolverType::from_str(solver, true)
                        .map_err(|message| anyhow::anyhow!(message))?,
                    ScoreCalculator::new(a, b, c, x),
                ))
            })
            .collect::<Result<HashMap<_, _>>>()?;

        Ok(Self(score_parameters))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_parameters_test() {
        let score_parameters = ScoreParameters::from_str(DEFAULT_SCORE_PARAMETERS).unwrap();
        assert_eq!(score_parameters.0.len(), 7);
    }
}
