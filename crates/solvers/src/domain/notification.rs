use {
    super::{
        auction,
        eth::{self, Ether, TokenAddress},
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

type SolutionId = u64;

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution(SolutionId),
    ScoringFailed,
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    SolverAccountInsufficientBalance(Ether),
    Settled(Settlement),
}

/// The result of winning solver trying to settle the transaction onchain.
#[derive(Debug)]
pub enum Settlement {
    Success(eth::H256),
    Revert(eth::H256),
    Fail,
}
