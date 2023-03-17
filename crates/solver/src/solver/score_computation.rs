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
    pub gas_amount_factor: f64,
    pub gas_price_factor: f64,
    pub nmb_orders_factor: f64,
    pub intercept: f64,
}

impl ScoreCalculator {
    pub fn calculate(&self, inputs: &objective_value::Inputs, nmb_orders: usize) -> Option<U256> {
        let surplus = inputs.surplus_given.to_f64()?;
        let fees = inputs.solver_fees.to_f64()?;
        let gas_amount = inputs.gas_amount.to_f64()?;
        let gas_price = inputs.gas_price.to_f64()?;
        let exponent = self.intercept.neg()
            - self.gas_amount_factor * gas_amount / 1_000_000.
            - self.gas_price_factor * gas_price / 10_000_000_000.
            - self.nmb_orders_factor * nmb_orders as f64;
        let success_probability = 1. / (1. + exponent.exp());
        let score = success_probability * (surplus + fees) - gas_amount * gas_price;
        Some(U256::from_f64_lossy(score))
    }
}

// The code for collecting the data and training the model can be found here:
// https://github.com/cowprotocol/risk_adjusted_rewards
// The data for each solver can be found here.
// https://drive.google.com/drive/u/1/folders/19yoL808qkp_os3BpLIYQahI3mQrNyx5T
#[rustfmt::skip]
const DEFAULT_SCORE_PARAMETERS: &str = "\
    Naive,0.5604082285267333,0.00285114179288399,0.06499875450001853,3.3987949311136787;\
    Baseline,-0.24391894879979226,-0.05809501139187965,-0.000013222507455295696,4.27946195371547;\
    CowDexAg,-0.9613998308805674,-0.14435150204689684,0.13923418474574772,2.7565258390467178;\
    OneInch,-0.32674185936325467,-0.05930446215554123,-0.33031769043234466,3.144609301500272;\
    Paraswap,-0.7815504846264341,-0.06965336115721313,0.0701725936991023,3.2617622830143453;\
    ZeroEx,-1.399997494341399,-0.04522233479453635,0.11066085229796373,2.7150950015915676;\
    BalancerSor,-0.7070951919365344,-0.1841886790519467,0.34189609422313544,3.6849833670945027";

#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// Parameters for the score computation for each solver.
    /// The format is a list of semicolon separated solver parameters.
    /// Each solver parameter is a comma separated list of parameters:
    /// [solver name],[gas amount factor],[gas price factor],[number of orders
    /// factor],[intercept parameter]
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

impl FromStr for ScoreParameters {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // parse user provided parameters (or default if none are provided)
        let user_score_parameters = parse_calculators(s)?;
        // parse default parameters and override them with the ones provided by the user
        let score_parameters = parse_calculators(DEFAULT_SCORE_PARAMETERS)?
            .into_iter()
            .map(|(solver, default_score_parameter)| {
                (
                    solver,
                    user_score_parameters
                        .get(&solver)
                        .cloned()
                        .unwrap_or(default_score_parameter),
                )
            })
            .collect();
        Ok(Self(score_parameters))
    }
}

fn parse_calculators(s: &str) -> Result<HashMap<SolverType, ScoreCalculator>> {
    s.split(';')
        .map(|part| {
            let (solver, parameters) = part
                .split_once(',')
                .context("malformed solver score parameters")?;
            let mut parameters = parameters.split(',');
            let gas_amount_factor = parameters
                .next()
                .context("missing a parameter for score")?
                .parse()?;
            let gas_price_factor = parameters
                .next()
                .context("missing b parameter for score")?
                .parse()?;
            let nmb_orders_factor = parameters
                .next()
                .context("missing c parameter for score")?
                .parse()?;
            let intercept = parameters
                .next()
                .context("missing x parameter for score")?
                .parse()?;
            Ok((
                SolverType::from_str(solver, true).map_err(|message| anyhow::anyhow!(message))?,
                ScoreCalculator {
                    gas_amount_factor,
                    gas_price_factor,
                    nmb_orders_factor,
                    intercept,
                },
            ))
        })
        .collect::<Result<HashMap<_, _>>>()
}

#[cfg(test)]
mod tests {
    use {super::*, number_conversions::u256_to_big_rational};

    #[test]
    fn score_parameters_test() {
        let score_parameters = ScoreParameters::from_str(DEFAULT_SCORE_PARAMETERS).unwrap();
        assert_eq!(score_parameters.0.len(), 7);
    }

    #[test]
    fn compute_score_test() {
        // tx hash 0x201c948ad94d7f93ad2d3c13fa4b6bbd4270533fbfedcb8be60e68c8e709d2b6
        // objective_score = 251547381429604400
        let score_parameters = ScoreParameters::from_str(DEFAULT_SCORE_PARAMETERS).unwrap();
        let inputs = objective_value::Inputs {
            surplus_given: u256_to_big_rational(&U256::from(237248548166961920u128)),
            solver_fees: u256_to_big_rational(&U256::from(45972570277472210u128)),
            gas_amount: u256_to_big_rational(&U256::from(765096u128)),
            gas_price: u256_to_big_rational(&U256::from(41398382700u128)),
        };
        let nmb_orders = 1;
        let score = score_parameters
            .0
            .get(&SolverType::Paraswap)
            .unwrap()
            .calculate(&inputs, nmb_orders)
            .unwrap();
        assert_eq!(score, 228957825032329696u128.into());
    }
}
