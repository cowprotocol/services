// Suppose a solver generates a solution, and we compute the three variables:
// surplus, fees and gas_cost. Currently, the objective is then simply equal to
// surplus + fees - gas_cost. However, for CIP-20, we now have one additional
// step for each solver. So for each solver we have trained a revert risk model,
// which essentially is a function that computes the probability of the
// settlement not reverting, and is a function of 3 parameters.
// p (num_orders_in_sol, gas_used, gas_price).

use {
    super::SolverType,
    anyhow::{Context, Result},
    clap::{Parser, ValueEnum},
    num::ToPrimitive,
    std::{
        collections::HashMap,
        fmt::{Display, Formatter},
        ops::Neg,
        str::FromStr,
    },
};

#[derive(Debug, Default, Clone)]
pub struct RiskCalculator {
    pub gas_amount_factor: f64,
    pub gas_price_factor: f64,
    pub nmb_orders_factor: f64,
    pub intercept: f64,
}

impl RiskCalculator {
    pub fn calculate(&self, gas_amount: f64, gas_price: f64, nmb_orders: usize) -> Result<f64> {
        let gas_amount = gas_amount.to_f64().context("gas_amount conversion")?;
        let gas_price = gas_price.to_f64().context("gas_price conversion")?;
        let exponent = self.intercept.neg()
            - self.gas_amount_factor * gas_amount / 1_000_000.
            - self.gas_price_factor * gas_price / 10_000_000_000.
            - self.nmb_orders_factor * nmb_orders as f64;
        let success_probability = 1. / (1. + exponent.exp());
        tracing::trace!(
            ?gas_amount,
            ?gas_price,
            ?nmb_orders,
            ?exponent,
            ?success_probability,
            "risk calculation",
        );
        Ok(success_probability)
    }
}

// The code for collecting the data and training the model can be found here:
// https://github.com/cowprotocol/risk_adjusted_rewards
// The data for each solver can be found here.
// https://drive.google.com/drive/u/1/folders/19yoL808qkp_os3BpLIYQahI3mQrNyx5T
#[rustfmt::skip]
const DEFAULT_RISK_PARAMETERS: &str = "\
    Naive,0.5604082285267333,0.00285114179288399,0.06499875450001853,3.3987949311136787;\
    Baseline,-0.24391894879979226,-0.05809501139187965,-0.000013222507455295696,4.27946195371547;\
    OneInch,-0.32674185936325467,-0.05930446215554123,-0.33031769043234466,3.144609301500272;\
    Paraswap,-0.7815504846264341,-0.06965336115721313,0.0701725936991023,3.2617622830143453;\
    ZeroEx,-1.399997494341399,-0.04522233479453635,0.11066085229796373,2.7150950015915676;\
    BalancerSor,-0.7070951919365344,-0.1841886790519467,0.34189609422313544,3.6849833670945027";

#[derive(Debug, Parser)]
#[group(skip)]
pub struct Arguments {
    /// Parameters for the risk computation for each solver.
    /// The format is a list of semicolon separated solver parameters.
    /// Each solver parameter is a comma separated list of parameters:
    /// [solver name],[gas amount factor],[gas price factor],[number of orders
    /// factor],[intercept parameter]
    #[clap(long, env, default_value = DEFAULT_RISK_PARAMETERS)]
    risk_parameters: RiskParameters,
}

impl Arguments {
    pub fn get_calculator(&self, solver: SolverType) -> Option<RiskCalculator> {
        self.risk_parameters.0.get(&solver).cloned()
    }
}

impl Display for Arguments {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "risk_parameters: {:?}", self.risk_parameters)
    }
}

#[derive(Debug, Clone)]
pub struct RiskParameters(HashMap<SolverType, RiskCalculator>);

impl FromStr for RiskParameters {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // parse user provided parameters (or default if none are provided)
        let user_risk_parameters = parse_calculators(s)?;
        // parse default parameters and override them with the ones provided by the user
        let risk_parameters = parse_calculators(DEFAULT_RISK_PARAMETERS)?
            .into_iter()
            .map(|(solver, default_risk_parameter)| {
                (
                    solver,
                    user_risk_parameters
                        .get(&solver)
                        .cloned()
                        .unwrap_or(default_risk_parameter),
                )
            })
            .collect();
        Ok(Self(risk_parameters))
    }
}

fn parse_calculators(s: &str) -> Result<HashMap<SolverType, RiskCalculator>> {
    s.split(';')
        .map(|part| {
            let (solver, parameters) = part
                .split_once(',')
                .context("malformed solver risk parameters")?;
            let mut parameters = parameters.split(',');
            let gas_amount_factor = parameters
                .next()
                .context("missing a parameter for risk")?
                .parse()?;
            let gas_price_factor = parameters
                .next()
                .context("missing b parameter for risk")?
                .parse()?;
            let nmb_orders_factor = parameters
                .next()
                .context("missing c parameter for risk")?
                .parse()?;
            let intercept = parameters
                .next()
                .context("missing x parameter for risk")?
                .parse()?;
            Ok((
                SolverType::from_str(solver, true).map_err(|message| anyhow::anyhow!(message))?,
                RiskCalculator {
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
    use super::*;

    #[test]
    fn risk_parameters_test() {
        let risk_parameters = RiskParameters::from_str(DEFAULT_RISK_PARAMETERS).unwrap();
        assert_eq!(risk_parameters.0.len(), 6);
    }

    #[test]
    fn compute_success_probability_test() {
        // tx hash 0x201c948ad94d7f93ad2d3c13fa4b6bbd4270533fbfedcb8be60e68c8e709d2b6
        // objective_score = 251547381429604400
        // success_probability ends up being 0.9202405649482063
        let risk_parameters = RiskParameters::from_str(DEFAULT_RISK_PARAMETERS).unwrap();
        let gas_amount = 765096.;
        let gas_price = 41398382700.;
        let nmb_orders = 1;
        let success_probability = risk_parameters
            .0
            .get(&SolverType::Paraswap)
            .unwrap()
            .calculate(gas_amount, gas_price, nmb_orders)
            .unwrap();
        assert_eq!(success_probability, 0.9202405649482063);
    }
}
