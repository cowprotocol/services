use {
    super::{
        auction,
        eth::{self, Ether, TokenAddress},
        solution::{self},
    },
    std::collections::BTreeSet,
};

type RequiredEther = Ether;
type TokensUsed = BTreeSet<TokenAddress>;
type TransactionHash = eth::H256;
type Transaction = eth::Tx;
type BlockNo = u64;
pub type SimulationSucceededAtLeastOnce = bool;

/// The notification about important events happened in driver, that solvers
/// need to know about.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: auction::Id,
    pub solution_id: Option<Id>,
    pub kind: Kind,
}

#[derive(Debug, Clone)]
pub enum Id {
    Single(solution::Id),
    Merged(Vec<u64>),
}

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    Timeout,
    EmptySolution,
    DuplicatedSolutionId,
    SimulationFailed(BlockNo, Transaction, SimulationSucceededAtLeastOnce),
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed(TokensUsed),
    SolverAccountInsufficientBalance(RequiredEther),
    Settled(Settlement),
    DriverError(String),
    PostprocessingTimedOut,
}

/// The result of winning solver trying to settle the transaction onchain.
#[derive(Debug)]
pub enum Settlement {
    Success(TransactionHash),
    Revert(TransactionHash),
    SimulationRevert,
    Fail,
}

#[derive(Debug)]
pub enum ScoreKind {
    InvalidClearingPrices,
    InvalidExecutedAmount,
    MissingPrice(TokenAddress),
}
