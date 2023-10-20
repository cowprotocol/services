use {
    crate::domain::{
        competition::{auction, solution},
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
    EmptySolution {
        solution: solution::Id
    },
    /// No valid score could be computed for the solution.
    ScoringFailed {
        kind: score::Kind,
        solution: solution::Id,
    },
    /// Solution aimed to internalize tokens that are not considered safe to
    /// keep in the settlement contract.
    NonBufferableTokensUsed {
        tokens: BTreeSet<TokenAddress>,
        solution: solution::Id,
    },
    /// Solver don't have enough balance to submit the solution onchain.
    SolverAccountInsufficientBalance(Ether),
}

mod score {
    use crate::domain::competition::score::SuccessProbability;

    #[derive(Debug)]
    pub enum Kind {
        SuccessProbabilityOutOfRange(SuccessProbability),
        ObjectiveValueNonPositive,
        ScoreHigherThanObjective,
    }
}

