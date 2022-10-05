use crate::{
    driver::solver_settlements::{self, retain_mature_settlements, RatedSettlement},
    metrics::{SolverMetrics, SolverRunOutcome, SolverSimulationOutcome},
    settlement::{external_prices::ExternalPrices, PriceCheckTokens, Settlement},
    settlement_rater::{RatedSolverSettlement, SettlementRating},
    solver::{SettlementWithError, Solver, SolverRunError},
};
use anyhow::Result;
use gas_estimation::GasPrice1559;
use num::{rational::Ratio, BigInt, BigRational, CheckedDiv, FromPrimitive};
use rand::prelude::SliceRandom;
use std::{cmp::Ordering, sync::Arc, time::Duration};

type SolverResult = (Arc<dyn Solver>, Result<Vec<Settlement>, SolverRunError>);

pub struct SettlementRanker {
    pub metrics: Arc<dyn SolverMetrics>,
    pub settlement_rater: Arc<dyn SettlementRating>,
    // TODO: these should probably come from the autopilot to make the test parameters identical for
    // everyone.
    pub min_order_age: Duration,
    pub max_settlement_price_deviation: Option<Ratio<BigInt>>,
    pub token_list_restriction_for_price_checks: PriceCheckTokens,
    pub decimal_cutoff: i32,
}

impl SettlementRanker {
    /// Discards settlements without user orders and settlements which violate price checks.
    /// Logs info and updates metrics about the out come of this run loop for each solver.
    fn discard_illegal_settlements(
        &self,
        solver: &Arc<dyn Solver>,
        settlements: Result<Vec<Settlement>, SolverRunError>,
        external_prices: &ExternalPrices,
    ) -> Vec<Settlement> {
        let name = solver.name();
        match settlements {
            Ok(mut settlement) => {
                for settlement in &settlement {
                    tracing::debug!(solver_name = %name, ?settlement, "found solution");
                }

                // Do not continue with settlements that are empty or only liquidity orders.
                let settlement_count = settlement.len();
                settlement.retain(solver_settlements::has_user_order);
                if settlement_count != settlement.len() {
                    tracing::debug!(
                        solver_name = %name,
                        "settlement(s) filtered containing only liquidity orders",
                    );
                }

                if let Some(max_settlement_price_deviation) = &self.max_settlement_price_deviation {
                    let settlement_count = settlement.len();
                    settlement.retain(|settlement| {
                        settlement.satisfies_price_checks(
                            solver.name(),
                            external_prices,
                            max_settlement_price_deviation,
                            &self.token_list_restriction_for_price_checks,
                        )
                    });
                    if settlement_count != settlement.len() {
                        tracing::debug!(
                            solver_name = %name,
                            "settlement(s) filtered for violating maximum external price deviation",
                        );
                    }
                }

                let outcome = match settlement.is_empty() {
                    true => SolverRunOutcome::Empty,
                    false => SolverRunOutcome::Success,
                };
                self.metrics.solver_run(outcome, name);

                settlement
            }
            Err(err) => {
                let outcome = match err {
                    SolverRunError::Timeout => SolverRunOutcome::Timeout,
                    SolverRunError::Solving(_) => SolverRunOutcome::Failure,
                };
                self.metrics.solver_run(outcome, name);
                tracing::warn!(solver_name = %name, ?err, "solver error");
                vec![]
            }
        }
    }

    /// Computes a list of settlements which pass all pre-simulation sanity checks.
    fn get_legal_settlements(
        &self,
        settlements: Vec<SolverResult>,
        prices: &ExternalPrices,
    ) -> Vec<(Arc<dyn Solver>, Settlement)> {
        let mut solver_settlements = vec![];
        for (solver, settlements) in settlements {
            let settlements = self.discard_illegal_settlements(&solver, settlements, prices);
            for settlement in settlements {
                solver_settlements.push((solver.clone(), settlement));
            }
        }

        // TODO this needs to move into the autopilot eventually.
        // filters out all non-mature settlements
        retain_mature_settlements(self.min_order_age, solver_settlements)
    }

    /// Determines legal settlements and ranks them by simulating them.
    /// Settlements get partitioned into simulation errors and a list
    /// of `RatedSettlement`s sorted by ascending order of objective value.
    pub async fn rank_legal_settlements(
        &self,
        settlements: Vec<SolverResult>,
        external_prices: &ExternalPrices,
        gas_price: GasPrice1559,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SettlementWithError>)> {
        let solver_settlements = self.get_legal_settlements(settlements, external_prices);

        // log considered settlements. While we already log all found settlements, this additonal
        // statement allows us to figure out which settlements were filtered out and which ones are
        // going to be simulated and considered for competition.
        for (solver, settlement) in &solver_settlements {
            tracing::debug!(
                solver_name = %solver.name(), ?settlement,
                "considering solution for solver competition",
            );
        }

        let (mut rated_settlements, errors) = self
            .settlement_rater
            .rate_settlements(solver_settlements, external_prices, gas_price)
            .await?;

        // Before sorting, make sure to shuffle the settlements. This is to make sure we don't give
        // preference to any specific solver when there is an objective value tie.
        rated_settlements.shuffle(&mut rand::thread_rng());

        rated_settlements.sort_by(|a, b| compare_solutions(&a.1, &b.1, self.decimal_cutoff));

        tracing::info!(
            "{} settlements passed simulation and {} failed",
            rated_settlements.len(),
            errors.len(),
        );
        for (solver, _, _) in &rated_settlements {
            self.metrics
                .settlement_simulation(solver.name(), SolverSimulationOutcome::Success);
        }

        Ok((rated_settlements, errors))
    }
}

fn compare_solutions(lhs: &RatedSettlement, rhs: &RatedSettlement, decimals: i32) -> Ordering {
    let precision = BigRational::from_i8(10).unwrap().pow(decimals);
    let rounded_lhs = (lhs
        .objective_value()
        .checked_div(&precision)
        .expect("precision cannot be 0"))
    .floor();
    let rounded_rhs = (rhs
        .objective_value()
        .checked_div(&precision)
        .expect("precision cannot be 0"))
    .floor();
    rounded_lhs.cmp(&rounded_rhs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethcontract::U256;
    use num::Zero;

    use crate::driver::solver_settlements::RatedSettlement;

    impl RatedSettlement {
        fn with_objective(objective_value: f64) -> Self {
            Self {
                id: 42,
                settlement: Default::default(),
                surplus: BigRational::from_float(objective_value).unwrap(),
                unscaled_subsidized_fee: Zero::zero(),
                scaled_unsubsidized_fee: Zero::zero(),
                gas_estimate: U256::zero(),
                gas_price: Zero::zero(),
            }
        }
    }

    #[test]
    fn compare_solutions_precise() {
        let better = RatedSettlement::with_objective(77495164315950.95);
        let worse = RatedSettlement::with_objective(77278255312878.95);
        assert_eq!(compare_solutions(&better, &worse, 0), Ordering::Greater);
        assert_eq!(compare_solutions(&worse, &better, 0), Ordering::Less);
        assert_eq!(compare_solutions(&better, &better, 0), Ordering::Equal);
    }

    #[test]
    fn compare_solutions_rounded() {
        let better = RatedSettlement::with_objective(0.16);
        let worse = RatedSettlement::with_objective(0.12);
        assert_eq!(compare_solutions(&better, &worse, -2), Ordering::Greater);
        assert_eq!(compare_solutions(&worse, &better, -2), Ordering::Less);
        assert_eq!(compare_solutions(&better, &worse, -1), Ordering::Equal);

        let worst = RatedSettlement::with_objective(0.09);
        assert_eq!(compare_solutions(&worse, &worst, -1), Ordering::Greater);

        let better = RatedSettlement::with_objective(77495164315950.95);
        let worse = RatedSettlement::with_objective(77278255312878.95);
        assert_eq!(compare_solutions(&better, &worse, 12), Ordering::Equal);
        assert_eq!(compare_solutions(&better, &worse, 11), Ordering::Greater);
    }
}
