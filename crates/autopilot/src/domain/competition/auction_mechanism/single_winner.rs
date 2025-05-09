use {
    super::{AuctionMechanism, ComputedScores, NoWinners, Participant, ReferenceScore, Unranked},
    crate::{
        domain::{Auction, competition::TradedOrder},
        infra,
    },
    primitive_types::U256,
    std::collections::{HashMap, HashSet},
};

pub struct SingleSurplusAuctionMechanism {
    pub eth: infra::Ethereum,
    pub max_solutions_per_solver: usize,
    pub max_winners_per_auction: usize,
}

impl SingleSurplusAuctionMechanism {
    /// Returns true if solution is fair to other solutions
    fn is_solution_fair(
        solution: &Participant<Unranked>,
        others: &[Participant<Unranked>],
        auction: &Auction,
    ) -> bool {
        let Some(fairness_threshold) = solution.driver().fairness_threshold else {
            return true;
        };

        // Returns the surplus difference in the buy token if `left`
        // is better for the trader than `right`, or 0 otherwise.
        // This takes differently partial fills into account.
        let improvement_in_buy = |left: &TradedOrder, right: &TradedOrder| {
            // If `left.sell / left.buy < right.sell / right.buy`, left is "better" as the
            // trader either sells less or gets more. This can be reformulated as
            // `right.sell * left.buy > left.sell * right.buy`.
            let right_sell_left_buy = right.executed_sell.0.full_mul(left.executed_buy.0);
            let left_sell_right_buy = left.executed_sell.0.full_mul(right.executed_buy.0);
            let improvement = right_sell_left_buy
                .checked_sub(left_sell_right_buy)
                .unwrap_or_default();

            // The difference divided by the original sell amount is the improvement in buy
            // token. Casting to U256 is safe because the difference is smaller than the
            // original product, which if re-divided by right.sell must fit in U256.
            improvement
                .checked_div(right.executed_sell.0.into())
                .map(|v| U256::try_from(v).expect("improvement in buy fits in U256"))
                .unwrap_or_default()
        };

        // Record best execution per order
        let mut best_executions = HashMap::new();
        for other in others {
            for (uid, execution) in other.solution().orders() {
                best_executions
                    .entry(uid)
                    .and_modify(|best_execution| {
                        if !improvement_in_buy(execution, best_execution).is_zero() {
                            *best_execution = *execution;
                        }
                    })
                    .or_insert(*execution);
            }
        }

        // Check if the solution contains an order whose execution in the
        // solution is more than `fairness_threshold` worse than the
        // order's best execution across all solutions
        let unfair = solution
            .solution()
            .orders()
            .iter()
            .any(|(uid, current_execution)| {
                let best_execution = best_executions.get(uid).expect("by construction above");
                let improvement = improvement_in_buy(best_execution, current_execution);
                if improvement.is_zero() {
                    return false;
                };
                tracing::debug!(
                    ?uid,
                    ?improvement,
                    ?best_execution,
                    ?current_execution,
                    "fairness check"
                );
                // Improvement is denominated in buy token, use buy price to normalize the
                // difference into eth
                let Some(order) = auction.orders.iter().find(|order| order.uid == *uid) else {
                    // This can happen for jit orders
                    tracing::debug!(?uid, "cannot ensure fairness, order not found in auction");
                    return false;
                };
                let Some(buy_price) = auction.prices.get(&order.buy.token) else {
                    tracing::warn!(
                        ?order,
                        "cannot ensure fairness, buy price not found in auction"
                    );
                    return false;
                };
                buy_price.in_eth(improvement.into()) > fairness_threshold
            });
        !unfair
    }
}

impl AuctionMechanism for SingleSurplusAuctionMechanism {
    fn filter_solutions(
        &self,
        auction: &Auction,
        solutions: &[Participant<Unranked>],
    ) -> Vec<Participant<Unranked>> {
        // Limit the number of accepted solutions per solver. Do not alter the ordering
        // of solutions
        let mut counter = HashMap::new();

        let solutions: Vec<_> = solutions
            .iter()
            .filter(|&participant| {
                let driver = participant.driver().name.clone();
                let count = counter.entry(driver).or_insert(0);
                *count += 1;
                *count <= self.max_solutions_per_solver
            })
            .cloned()
            .collect();

        // Fairness check
        solutions
            .iter()
            .enumerate()
            .filter_map(|(index, participant)| {
                if Self::is_solution_fair(participant, &solutions[index..], auction) {
                    Some(participant.clone())
                } else {
                    tracing::warn!(
                        invalidated = participant.driver().name,
                        "fairness check invalidated of solution"
                    );
                    None
                }
            })
            .collect()
    }

    /// Winners are selected one by one, starting from the best solution,
    /// until `max_winners_per_auction` are selected. The solution is a winner
    /// if it swaps tokens that are not yet swapped by any previously processed
    /// solution.
    fn select_winners(&self, solutions: &[Participant<Unranked>]) -> Vec<Participant> {
        let wrapped_native_token = self.eth.contracts().wrapped_native_token();
        let mut already_swapped_tokens = HashSet::new();
        let mut winners = 0;
        solutions
            .iter()
            .map(|participant| {
                let swapped_tokens = participant
                    .solution()
                    .orders()
                    .iter()
                    .flat_map(|(_, order)| {
                        [
                            order.sell.token.as_erc20(wrapped_native_token),
                            order.buy.token.as_erc20(wrapped_native_token),
                        ]
                    })
                    .collect::<HashSet<_>>();

                let is_winner = swapped_tokens.is_disjoint(&already_swapped_tokens)
                    && winners < self.max_winners_per_auction;

                already_swapped_tokens.extend(swapped_tokens);
                winners += usize::from(is_winner);

                participant.clone().rank(is_winner)
            })
            .collect()
    }

    fn compute_scores(&self, solutions: &[Participant]) -> Result<ComputedScores, NoWinners> {
        let Some(winning_solution) = solutions
            .iter()
            .find(|participant| participant.is_winner())
            .map(|participant| participant.solution())
        else {
            return Err(NoWinners);
        };
        let winner = winning_solution.solver().into();
        let winning_score = winning_solution.score().get().0;
        let reference_score = solutions
            .get(1)
            .map(|participant| participant.solution().score().get().0)
            .unwrap_or_default();

        Ok(ComputedScores {
            winner,
            winning_score,
            reference_scores: vec![ReferenceScore {
                solver: winner,
                reference_score,
            }],
        })
    }
}
