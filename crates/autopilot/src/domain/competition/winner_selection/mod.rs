use crate::domain::competition::{Participant, Ranked, Unranked};

pub mod combinatorial;

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
