use {
    crate::domain::{
        Auction,
        competition::{Participant, Ranked, Score, Unranked},
        eth,
    },
    std::collections::HashMap,
};

pub mod combinatorial;
pub mod max_score;

pub struct Ranking {
    /// Solutions that were discarded because they were malformed
    /// in some way or deemed unfair by the selection mechanism.
    filtered_out: Vec<Participant<Ranked>>,
    /// Final ranking of the solutions that passed the fairness
    /// check. Winners come before non-winners and higher total
    /// scores come before lower scores.
    ranked: Vec<Participant<Ranked>>,
}

impl Ranking {
    /// All solutions including the ones that got filtered out.
    pub fn all(&self) -> impl Iterator<Item = &Participant<Ranked>> {
        self.ranked.iter().chain(&self.filtered_out)
    }

    /// Enumerates all solutions. The index is used as solution UID.
    pub fn enumerated(&self) -> impl Iterator<Item = (usize, &Participant<Ranked>)> {
        self.all().enumerate()
    }

    /// All solutions that won the right to get executed.
    pub fn winners(&self) -> impl Iterator<Item = &Participant<Ranked>> {
        self.ranked.iter().filter(|p| p.is_winner())
    }

    /// All solutions that were not filtered out but also did not win.
    pub fn non_winners(&self) -> impl Iterator<Item = &Participant<Ranked>> {
        self.ranked.iter().filter(|p| !p.is_winner())
    }

    /// All solutions that passed the filtering step.
    pub fn ranked(&self) -> impl Iterator<Item = &Participant<Ranked>> {
        self.ranked.iter()
    }
}

pub struct PartitionedSolutions {
    kept: Vec<Participant<Unranked>>,
    discarded: Vec<Participant<Unranked>>,
}

/// Implements auction arbitration in 3 phases:
/// 1. filter unfair solutions
/// 2. mark winners
/// 3. compute reference scores
///
/// The functions assume the `Arbitrator` is the only one
/// changing the ordering or the `participants.
pub trait Arbitrator: Send + Sync + 'static {
    /// Runs the entire auction mechanism on the passed in solutions.
    fn arbitrate(&self, participants: Vec<Participant<Unranked>>, auction: &Auction) -> Ranking {
        let partitioned = self.partition_unfair_solutions(participants, auction);
        let filtered_out = partitioned
            .discarded
            .into_iter()
            .map(|participant| participant.rank(Ranked::FilteredOut))
            .collect();

        let mut ranked = self.mark_winners(partitioned.kept);
        ranked.sort_by_key(|participant| {
            (
                // winners before non-winners
                std::cmp::Reverse(participant.is_winner()),
                // high score before low score
                std::cmp::Reverse(participant.solution().computed_score().cloned()),
            )
        });
        Ranking {
            filtered_out,
            ranked,
        }
    }

    /// Removes unfair solutions from the set of all solutions.
    fn partition_unfair_solutions(
        &self,
        participants: Vec<Participant<Unranked>>,
        auction: &Auction,
    ) -> PartitionedSolutions;

    /// Picks winners and sorts all solutions where winners come before
    /// non-winners and higher scores come before lower scores.
    fn mark_winners(&self, participants: Vec<Participant<Unranked>>) -> Vec<Participant<Ranked>>;

    /// Computes the reference scores which are used to compute
    /// rewards for the winning solvers.
    fn compute_reference_scores(&self, ranking: &Ranking) -> HashMap<eth::Address, Score>;
}
