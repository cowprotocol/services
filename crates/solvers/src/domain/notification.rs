use {
    super::{
        auction,
        eth::{self, Ether, TokenAddress},
        solution::SuccessProbability,
    },
    std::collections::BTreeSet,
};

/// The notification about important events happened in driver, that solvers
/// need to know about.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: auction::Id,
    pub kind: Kind,
}

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution,
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    SolverAccountInsufficientBalance(Ether),
}

#[derive(Debug)]
pub enum ScoreKind {
    SuccessProbabilityOutOfRange(SuccessProbability),
    ObjectiveValueNonPositive,
    ScoreHigherThanObjective(Score),
}

#[derive(Debug, Copy, Clone)]
pub struct Score(pub eth::U256);

impl From<eth::U256> for Score {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}
