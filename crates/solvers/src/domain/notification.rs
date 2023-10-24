use {
    super::{
        auction,
        eth::{self, Ether, TokenAddress},
        solution::{self, SuccessProbability},
    },
    std::collections::BTreeSet,
};

/// The notification about important events happened in driver, that solvers
/// need to know about.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: auction::Id,
    pub solution_id: solution::Id,
    pub kind: Kind,
}

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution,
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    SolverAccountInsufficientBalance(Ether),
    DuplicatedSolutionId,
}

#[derive(Debug)]
pub enum ScoreKind {
    ZeroScore,
    ObjectiveValueNonPositive,
    SuccessProbabilityOutOfRange(SuccessProbability),
    ScoreHigherThanObjective(Score, ObjectiveValue),
}

#[derive(Debug, Copy, Clone)]
pub struct Score(pub eth::U256);

impl From<eth::U256> for Score {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct ObjectiveValue(pub eth::U256);

impl From<eth::U256> for ObjectiveValue {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}
