use {
    super::{
        auction,
        eth::{self, Ether, TokenAddress},
        solution::{self, SuccessProbability},
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
    ZeroScore,
    ScoreHigherThanQuality(Score, Quality),
    SuccessProbabilityOutOfRange(SuccessProbability),
    ObjectiveValueNonPositive(Quality, GasCost),
}

#[derive(Debug, Copy, Clone)]
pub struct Score(pub eth::U256);

impl From<eth::U256> for Score {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct Quality(pub eth::U256);

impl From<eth::U256> for Quality {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct GasCost(pub eth::U256);

impl From<eth::U256> for GasCost {
    fn from(value: eth::U256) -> Self {
        Self(value)
    }
}
