use {
    crate::domain::{
        competition::{
            self,
            auction,
            score::{ObjectiveValue, SuccessProbability},
        },
        eth::{Ether, TokenAddress},
    },
    std::collections::BTreeSet,
};

/// A notification is sent to the solvers in case a solution failed validation.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: Option<auction::Id>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    /// The solution doesn't contain any user orders.
    EmptySolution,
    /// No valid score could be computed for the solution.
    ScoringFailed(ScoreKind),
    /// Solution aimed to internalize tokens that are not considered safe to
    /// keep in the settlement contract.
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    /// Solver don't have enough balance to submit the solution onchain.
    SolverAccountInsufficientBalance(Ether),
}

#[derive(Debug)]
pub enum ScoreKind {
    SuccessProbabilityOutOfRange(SuccessProbability),
    ObjectiveValueNonPositive(ObjectiveValue),
    ScoreHigherThanObjective(competition::Score),
}
