//! Winner Selection:
//! Simply picks the 1 solution with the highest overall score.
//!
//! Fairness Guarantees:
//! This winner selection does not have any inherent fairness guarantees for
//! individual orders. However, each order's execution is basically "insured" by
//! EBBO (ethereum best bid offer) - that is an order should get executed at
//! least as good as possible using very popular onchain liquidity sources. Each
//! solver can opt-in to have their solutions invalidated if the estimated total
//! EBBO violations would exceed a configurable threshold.
//!
//! Reference Score:
//! The reference score is simply the second highest reported score of all
//! solutions. If there is only 1 solution the reference score is 0.
use {
    super::{Arbitrator, PartitionedSolutions, Ranking},
    crate::domain::{
        Auction,
        competition::{Participant, Ranked, Score, TradedOrder, Unranked},
        eth,
    },
    ethcontract::U256,
    itertools::{Either, Itertools},
    std::collections::HashMap,
};

pub struct Config;

impl Arbitrator for Config {
    fn partition_unfair_solutions(
        &self,
        mut participants: Vec<Participant<Unranked>>,
        auction: &Auction,
    ) -> PartitionedSolutions {
        // sort by score descending
        participants.sort_unstable_by_key(|participant| {
            std::cmp::Reverse(participant.solution().score().get().0)
        });
        let (fair, unfair) =
            participants
                .iter()
                .enumerate()
                .partition_map(|(index, participant)| {
                    if is_solution_fair(participant, &participants[index..], auction) {
                        Either::Left(participant.clone())
                    } else {
                        tracing::warn!(
                            invalidated = participant.driver().name,
                            "fairness check invalidated of solution"
                        );
                        Either::Right(participant.clone())
                    }
                });
        PartitionedSolutions {
            discarded: unfair,
            kept: fair,
        }
    }

    fn mark_winners(&self, participants: Vec<Participant<Unranked>>) -> Vec<Participant> {
        // The current system theoretically already supports multiple winners. However,
        // it was never activated because the rewards mechanism was never
        // decided. To make the migration easier we revert back to only allowing
        // 1 winner. And that is simply the solution with the highest total
        // score.
        participants
            .into_iter()
            .enumerate()
            .map(|(index, participant)| {
                let rank = match index == 0 {
                    true => Ranked::Winner,
                    false => Ranked::NonWinner,
                };
                participant.rank(rank)
            })
            .collect()
    }

    fn compute_reference_scores(&self, ranking: &Ranking) -> HashMap<eth::Address, Score> {
        // this will hold at most 1 score but the interface needs to support multiple
        // scores to fit the interface
        let mut reference_scores = HashMap::default();
        if let Some(winner) = ranking.ranked.first() {
            let runner_up = ranking
                .ranked
                .get(1)
                .map(|s| s.solution().score())
                .unwrap_or_default();
            reference_scores.insert(winner.driver().submission_address, runner_up);
        }
        reference_scores
    }
}

/// Returns true if solution is fair to other solutions.
fn is_solution_fair(
    participant: &Participant<Unranked>,
    others: &[Participant<Unranked>],
    auction: &Auction,
) -> bool {
    let Some(fairness_threshold) = participant.driver().fairness_threshold else {
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
    let unfair = participant
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
