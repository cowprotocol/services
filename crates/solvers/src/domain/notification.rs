use {
    super::{auction, eth::TokenAddress},
    std::collections::BTreeSet,
};

/// The notification about important events happened in driver, that solvers
/// need to know about.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: auction::Id,
    pub kind: Kind,
}

type SolutionId = u64;

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution(SolutionId),
    ScoringFailed,
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    SolverAccountInsufficientBalance,
}
