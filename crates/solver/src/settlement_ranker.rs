use {
    crate::{
        driver::solver_settlements::{self, RatedSettlement},
        metrics::{SolverMetrics, SolverRunOutcome, SolverSimulationOutcome},
        settlement::{external_prices::ExternalPrices, PriceCheckTokens, Settlement},
        settlement_rater::{RatedSolverSettlement, SettlementRating},
        settlement_simulation::call_data,
        solver::{SimulationWithError, Solver},
    },
    anyhow::Result,
    gas_estimation::GasPrice1559,
    itertools::enumerate,
    model::auction::AuctionId,
    num::{rational::Ratio, BigInt, BigRational, CheckedDiv, FromPrimitive},
    rand::prelude::SliceRandom,
    shared::http_solver::model::{
        AuctionResult,
        InternalizationStrategy,
        SolverRejectionReason,
        SolverRunError,
        TransactionWithError,
    },
    std::{cmp::Ordering, sync::Arc},
};

type SolverResult = (Arc<dyn Solver>, Result<Vec<Settlement>, SolverRunError>);

// We require from solvers to have a bit more ETH balance then needed
// at the moment of simulating the transaction, to cover the potential increase
// of the cost of sending transaction onchain, because of the sudden gas price
// increase. To simulate this sudden increase of gas price during simulation, we
// artificially multiply the gas price with this factor.
const SOLVER_BALANCE_MULTIPLIER: f64 = 3.;
pub struct SettlementRanker {
    pub metrics: Arc<dyn SolverMetrics>,
    pub settlement_rater: Arc<dyn SettlementRating>,
    // TODO: these should probably come from the autopilot to make the test parameters identical
    // for everyone.
    pub max_settlement_price_deviation: Option<Ratio<BigInt>>,
    pub token_list_restriction_for_price_checks: PriceCheckTokens,
    pub decimal_cutoff: u16,
}

impl SettlementRanker {
    /// Discards settlements without user orders and settlements which violate
    /// price checks. Logs info and updates metrics about the out come of
    /// this run loop for each solver.
    fn discard_illegal_settlements(
        &self,
        solver: &Arc<dyn Solver>,
        settlements: Result<Vec<Settlement>, SolverRunError>,
        external_prices: &ExternalPrices,
        auction_id: AuctionId,
    ) -> Vec<Settlement> {
        let name = solver.name();
        match settlements {
            Ok(settlements) => {
                let settlements: Vec<_> = settlements.into_iter().filter_map(|settlement| {
                    tracing::debug!(solver_name = %name, ?settlement, "found solution");

                    // Do not continue with settlements that are empty or only liquidity orders.
                    if !solver_settlements::has_user_order(&settlement) {
                        tracing::debug!(
                            solver_name = %name,
                            "settlement(s) filtered containing only liquidity orders",
                        );
                        solver.notify_auction_result(auction_id, AuctionResult::Rejected(SolverRejectionReason::NoUserOrders));
                        return None;
                    }

                    // Do not continue with settlements that contain prices too different from external prices.
                    if let Some(max_settlement_price_deviation) = &self.max_settlement_price_deviation {
                        if !
                            settlement.satisfies_price_checks(
                                solver.name(),
                                external_prices,
                                max_settlement_price_deviation,
                                &self.token_list_restriction_for_price_checks,
                            ) {

                                tracing::debug!(
                                    solver_name = %name,
                                    "settlement(s) filtered for violating maximum external price deviation",
                                );

                                solver.notify_auction_result(auction_id, AuctionResult::Rejected(SolverRejectionReason::PriceViolation));
                                return None;
                            }
                    }

                    Some(settlement)
                }).collect();

                let outcome = match settlements.is_empty() {
                    true => SolverRunOutcome::Empty,
                    false => SolverRunOutcome::Success,
                };
                self.metrics.solver_run(outcome, name);
                settlements
            }
            Err(err) => {
                let outcome = match err {
                    SolverRunError::Timeout => SolverRunOutcome::Timeout,
                    SolverRunError::Solving(_) => SolverRunOutcome::Failure,
                };
                self.metrics.solver_run(outcome, name);
                tracing::warn!(solver_name = %name, ?err, "solver error");
                solver.notify_auction_result(
                    auction_id,
                    AuctionResult::Rejected(SolverRejectionReason::RunError(err)),
                );
                vec![]
            }
        }
    }

    /// Computes a list of settlements which pass all pre-simulation sanity
    /// checks.
    fn get_legal_settlements(
        &self,
        settlements: Vec<SolverResult>,
        prices: &ExternalPrices,
        auction_id: AuctionId,
    ) -> Vec<(Arc<dyn Solver>, Settlement)> {
        let mut solver_settlements = vec![];
        for (solver, settlements) in settlements {
            let settlements =
                self.discard_illegal_settlements(&solver, settlements, prices, auction_id);
            for settlement in settlements {
                solver_settlements.push((solver.clone(), settlement));
            }
        }
        solver_settlements
    }

    /// Determines legal settlements and ranks them by simulating them.
    /// Settlements get partitioned into simulation errors and a list
    /// of `RatedSettlement`s sorted by ascending order of objective value.
    pub async fn rank_legal_settlements(
        &self,
        settlements: Vec<SolverResult>,
        external_prices: &ExternalPrices,
        gas_price: GasPrice1559,
        auction_id: AuctionId,
    ) -> Result<(Vec<RatedSolverSettlement>, Vec<SimulationWithError>)> {
        let gas_price = gas_price.bump(SOLVER_BALANCE_MULTIPLIER);

        let solver_settlements =
            self.get_legal_settlements(settlements, external_prices, auction_id);

        // log considered settlements. While we already log all found settlements, this
        // additonal statement allows us to figure out which settlements were
        // filtered out and which ones are going to be simulated and considered
        // for competition.
        for (solver, settlement) in &solver_settlements {
            let uninternalized_calldata = format!(
                "0x{}",
                hex::encode(call_data(
                    settlement
                        .clone()
                        .encode(InternalizationStrategy::EncodeAllInteractions)
                )),
            );

            tracing::debug!(
                solver_name = %solver.name(), ?settlement, %uninternalized_calldata,
                "considering solution for solver competition",
            );
        }

        let (mut rated_settlements, errors) = self
            .settlement_rater
            .rate_settlements(solver_settlements, external_prices, gas_price)
            .await?;

        // Before sorting, make sure to shuffle the settlements. This is to make sure we
        // don't give preference to any specific solver when there is an
        // objective value tie.
        rated_settlements.shuffle(&mut rand::thread_rng());

        rated_settlements.sort_by(|a, b| compare_solutions(&a.1, &b.1, self.decimal_cutoff));

        tracing::info!(
            "{} settlements passed simulation and {} failed",
            rated_settlements.len(),
            errors.len(),
        );
        for error in &errors {
            error.simulation.solver.notify_auction_result(
                auction_id,
                AuctionResult::Rejected(SolverRejectionReason::SimulationFailure(
                    TransactionWithError {
                        transaction: error.simulation.transaction.clone(),
                        error: error.error.to_string(),
                    },
                )),
            );
        }
        for (i, (solver, _, _)) in enumerate(&rated_settlements) {
            let rank = rated_settlements.len() - i;
            solver.notify_auction_result(auction_id, AuctionResult::Ranked(rank));
            self.metrics
                .settlement_simulation(solver.name(), SolverSimulationOutcome::Success);
        }

        Ok((rated_settlements, errors))
    }
}

fn compare_solutions(lhs: &RatedSettlement, rhs: &RatedSettlement, decimals: u16) -> Ordering {
    let precision = BigRational::from_i8(10).unwrap().pow(decimals.into());
    let rounded_lhs = lhs
        .objective_value
        .checked_div(&precision)
        .expect("precision cannot be 0")
        .floor();
    let rounded_rhs = rhs
        .objective_value
        .checked_div(&precision)
        .expect("precision cannot be 0")
        .floor();
    rounded_lhs.cmp(&rounded_rhs)
}

#[cfg(test)]
mod tests {
    use {super::*, crate::driver::solver_settlements::RatedSettlement};

    impl RatedSettlement {
        fn with_objective(objective_value: f64) -> Self {
            Self {
                objective_value: BigRational::from_f64(objective_value).unwrap(),
                ..Default::default()
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
        let better = RatedSettlement::with_objective(77495164315950.95);
        let worse = RatedSettlement::with_objective(77278255312878.95);
        assert_eq!(compare_solutions(&better, &worse, 12), Ordering::Equal);
        assert_eq!(compare_solutions(&better, &worse, 11), Ordering::Greater);
    }
}
