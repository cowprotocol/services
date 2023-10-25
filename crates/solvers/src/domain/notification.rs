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

pub type RequiredEther = Ether;
pub type TokensUsed = BTreeSet<TokenAddress>;

/// All types of notifications solvers can be informed about.
#[derive(Debug)]
pub enum Kind {
    EmptySolution,
    DuplicatedSolutionId,
    ScoringFailed(ScoreKind),
    NonBufferableTokensUsed(TokensUsed),
    SolverAccountInsufficientBalance(RequiredEther),
    Settled(Settlement),
}

pub type TransactionHash = eth::H256;

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
