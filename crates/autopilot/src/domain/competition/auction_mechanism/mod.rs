mod single_winner;

pub use single_winner::SingleSurplusAuctionMechanism;
use {
    super::{Participant, Unranked},
    crate::domain::Auction,
    primitive_types::{H160, U256},
};

#[derive(Debug, thiserror::Error)]
#[error("no winners found")]
pub struct NoWinners;

#[derive(Clone, Default, Debug)]
pub struct ComputedScores {
    // TODO: for now we specify a single winner as the database still expectes it
    // After https://github.com/cowprotocol/services/issues/3350, it will no longer be necessary and we will be able to return only the vec of ReferenceScore
    pub winner: H160,
    pub winning_score: U256,
    pub reference_scores: Vec<ReferenceScore>,
}

#[derive(Clone, Default, Debug)]
pub struct ReferenceScore {
    pub solver: H160,
    pub reference_score: U256,
}

/// The following trait allows to implement custom auction mechanism logic
/// for competitions.
pub trait AuctionMechanism: Send + Sync {
    /// Filters out invalid or unfair solutions.
    fn filter_solutions(
        &self,
        auction: &Auction,
        solutions: &[Participant<Unranked>],
    ) -> Vec<Participant<Unranked>>;

    /// Selects the winners from a list of unranked solutions.
    ///
    /// Returns the list of solutions with the winners marked.
    fn select_winners(&self, solutions: &[Participant<Unranked>]) -> Vec<Participant>;

    /// Computes the scores of all provided solutions.
    fn compute_scores(&self, solutions: &[Participant]) -> Result<ComputedScores, NoWinners>;
}
