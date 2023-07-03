use {
    crate::{
        driver::solver_settlements::{self, RatedSettlement},
        metrics::{SolverMetrics, SolverRunOutcome, SolverSimulationOutcome},
        settlement::{self, PriceCheckTokens, Settlement},
        settlement_rater::{Rating, SettlementRating},
        settlement_simulation::call_data,
        solver::{SimulationWithError, Solver, SolverInfo},
    },
    anyhow::Result,
    ethcontract::U256,
    futures::future::join_all,
    gas_estimation::GasPrice1559,
    itertools::Itertools,
    model::auction::AuctionId,
    num::{rational::Ratio, BigInt},
    number_conversions::big_rational_to_u256,
    rand::prelude::SliceRandom,
    shared::{
        external_prices::ExternalPrices,
        http_solver::model::{
            AuctionResult,
            InternalizationStrategy,
            SolverRejectionReason,
            SolverRunError,
            TransactionWithError,
        },
    },
    std::sync::Arc,
};

type SolverResult = (Arc<dyn Solver>, Result<Vec<Settlement>, SolverRunError>);
pub type RatedSolverSettlement = (Arc<dyn Solver>, RatedSettlement);

pub struct SettlementRanker {
    pub settlement_encoding_contracts: settlement::Contracts,
    pub metrics: Arc<dyn SolverMetrics>,
    pub settlement_rater: Arc<dyn SettlementRating>,
    // TODO: these should probably come from the autopilot to make the test parameters identical
    // for everyone.
    pub max_settlement_price_deviation: Option<Ratio<BigInt>>,
    pub token_list_restriction_for_price_checks: PriceCheckTokens,
    pub decimal_cutoff: u16,
    pub skip_non_positive_score_settlements: bool,
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
                        tracing::trace!(
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
        let solver_settlements =
            self.get_legal_settlements(settlements, external_prices, auction_id);

        // log considered settlements. While we already log all found settlements, this
        // additonal statement allows us to figure out which settlements were
        // filtered out and which ones are going to be simulated and considered
        // for competition.
        for (solver, settlement) in &solver_settlements {
            let uninternalized_calldata = format!(
                "0x{}",
                hex::encode(call_data(settlement.clone().encode(
                    &self.settlement_encoding_contracts,
                    InternalizationStrategy::EncodeAllInteractions
                ))),
            );

            tracing::debug!(
                solver_name = %solver.name(), ?settlement, %uninternalized_calldata,
                "considering solution for solver competition",
            );
        }

        let (mut rated_settlements, errors): (Vec<_>, Vec<_>) =
            join_all(solver_settlements.into_iter().enumerate().map(
                |(i, (solver, settlement))| async move {
                    let simulation = self
                        .settlement_rater
                        .rate_settlement(
                            &SolverInfo {
                                account: solver.account().clone(),
                                name: solver.name().to_owned(),
                            },
                            settlement,
                            external_prices,
                            gas_price,
                            i,
                        )
                        .await;
                    (solver, simulation)
                },
            ))
            .await
            .into_iter()
            .filter_map(|(solver, result)| match result {
                Ok(res) => Some((solver, res)),
                Err(err) => {
                    tracing::warn!(?err, "error in settlement rating logic");
                    None
                }
            })
            .partition_map(|(solver, result)| match result {
                Rating::Ok(r) => itertools::Either::Left((solver, r)),
                Rating::Err(err) => itertools::Either::Right((solver, err)),
            });

        tracing::info!(
            "{} settlements passed simulation and {} failed",
            rated_settlements.len(),
            errors.len(),
        );
        for (solver, error) in &errors {
            solver.notify_auction_result(
                auction_id,
                AuctionResult::Rejected(SolverRejectionReason::SimulationFailure(
                    TransactionWithError {
                        transaction: error.simulation.transaction.clone(),
                        error: error.error.to_string(),
                    },
                )),
            );
        }

        // Filter out settlements with non-positive score.
        if self.skip_non_positive_score_settlements {
            rated_settlements.retain(|(solver, settlement)| {
                let positive_score = settlement.score.score() > 0.into();
                if !positive_score {
                    tracing::debug!(
                        solver_name = %solver.name(),
                        "settlement filtered for having non-positive score",
                    );
                    solver.notify_auction_result(
                        auction_id,
                        AuctionResult::Rejected(SolverRejectionReason::NonPositiveScore),
                    );
                    self.metrics.settlement_non_positive_score(solver.name());
                }
                positive_score
            });
        }

        // Filter out settlements with too high score.
        rated_settlements.retain(|(solver, settlement)| {
            let surplus = big_rational_to_u256(&settlement.surplus).unwrap_or(U256::MAX);
            let fees = big_rational_to_u256(&settlement.solver_fees).unwrap_or(U256::MAX);
            let max_score = surplus.saturating_add(fees);
            let valid_score = settlement.score.score() < max_score;
            if !valid_score {
                tracing::debug!(
                    solver_name = %solver.name(),
                    "settlement filtered for having too high score",
                );
                solver.notify_auction_result(
                    auction_id,
                    AuctionResult::Rejected(SolverRejectionReason::TooHighScore {
                        surplus,
                        fees,
                        max_score,
                        submitted_score: settlement.score.score(),
                    }),
                );
            }
            valid_score
        });

        // Before sorting, make sure to shuffle the settlements. This is to make sure we
        // don't give preference to any specific solver when there is a score tie.
        rated_settlements.shuffle(&mut rand::thread_rng());
        rated_settlements.sort_by_key(|(_, settlement)| settlement.score.score());

        rated_settlements
            .iter_mut()
            .rev()
            .enumerate()
            .for_each(|(i, (solver, settlement))| {
                self.metrics
                    .settlement_simulation(solver.name(), SolverSimulationOutcome::Success);
                settlement.ranking = i + 1;
                solver.notify_auction_result(auction_id, AuctionResult::Ranked(i + 1));
            });
        let errors = errors.into_iter().map(|(_, error)| error).collect();
        Ok((rated_settlements, errors))
    }
}
