mod single_winner;

pub use single_winner::SingleSurplusAuctionMechanism;
use {
    super::{Participant, Unranked},
    crate::domain::Auction,
    primitive_types::{H160, U256},
    std::collections::{HashMap, hash_map::Iter},
};

#[derive(Debug, thiserror::Error)]
#[error("no winners found")]
pub struct NoWinners;

#[derive(Default)]
pub struct CompetitionData {
    // TODO: for now we specify a single winner as the database still expects it
    // After https://github.com/cowprotocol/services/issues/3350, it will no longer be necessary and we will be able to return only the vec of ReferenceScore
    pub legacy_scores: LegacyScores,
    pub solutions: Vec<Participant>,
    pub reference_scores: ReferenceScores,
}

impl CompetitionData {
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }
}

/// Legacy scores support only a single winner. This structure remains to avoid
/// breaking changes in the database schema. Will be removed in the future.
#[derive(Clone, Default, Debug)]
pub struct LegacyScores {
    pub winner: H160,
    pub winning_score: U256,
    pub reference_score: U256,
}

/// Contains reference scores per solver address.
#[derive(Clone, Default, Debug)]
pub struct ReferenceScores(HashMap<H160, U256>);

impl ReferenceScores {
    pub fn iter(&self) -> Iter<'_, H160, U256> {
        self.0.iter()
    }
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
    fn rank_solutions(&self, solutions: &[Participant<Unranked>]) -> Vec<Participant>;

    /// Computes competition data which includes:
    /// - Filtered and ranked solutions
    /// - Reference scores for each solver
    /// - Legacy scores for the winner
    fn compute_competition_data(
        &self,
        auction: &Auction,
        solutions: &[Participant<Unranked>],
    ) -> CompetitionData;
}
