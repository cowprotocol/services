use {
    super::{
        auction,
        eth::{Ether, TokenAddress},
        solution,
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

type SolutionId = u64;

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution,
    ScoringFailed,
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    SolverAccountInsufficientBalance(Ether),
    DuplicatedSolutionId,
}
