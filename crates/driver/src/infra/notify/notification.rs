use {
    crate::domain::{
        competition::{auction, solution, ObjectiveValue, Score, SuccessProbability},
        eth::{self, Ether, TokenAddress},
    },
    std::collections::BTreeSet,
};

/// A notification sent to solvers in case of important events in the driver.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: Option<auction::Id>,
    pub solution_id: Option<solution::Id>,
    pub kind: Kind,
}

pub type RequiredEther = Ether;
pub type TokensUsed = BTreeSet<TokenAddress>;

#[derive(Debug)]
pub enum Kind {
    /// Solver engine timed out.
    Timeout,
    /// The solution doesn't contain any user orders.
    EmptySolution,
    /// Solution received from solver engine don't have unique id.
    DuplicatedSolutionId,
    /// No valid score could be computed for the solution.
    ScoringFailed(ScoreKind),
    /// Solution aimed to internalize tokens that are not considered safe to
    /// keep in the settlement contract.
    NonBufferableTokensUsed(TokensUsed),
    /// Solver don't have enough balance to submit the solution onchain.
    SolverAccountInsufficientBalance(RequiredEther),
    /// Result of winning solver trying to settle the transaction onchain.
    Settled(Settlement),
}

#[derive(Debug)]
pub enum ScoreKind {
    /// The solution has zero score. Zero score solutions are not allowed as per
    /// CIP20 definition. The main reason being that reference score is zero,
    /// and if only one solution is in competition with zero score, that
    /// solution would receive 0 reward (reward = score - reference score).
    ZeroScore,
    /// Objective value is defined as surplus + fees - gas costs. Protocol
    /// doesn't allow solutions that cost more than they bring to the users and
    /// protocol.
    ObjectiveValueNonPositive,
    /// Solution has success probability that is outside of the allowed range
    /// [0, 1]
    SuccessProbabilityOutOfRange(SuccessProbability),
    /// Protocol does not allow solutions that are claimed to be "better" than
    /// the actual value they bring (objective value). It is expected that score
    /// is always lower than objective value, because there is always some
    /// revert risk that needs to be incorporated into the score and lower it.
    ScoreHigherThanObjective(Score, ObjectiveValue),
}

type TransactionHash = eth::TxId;

#[derive(Debug)]
pub enum Settlement {
    /// Winning solver settled successfully transaction onchain.
    Success(TransactionHash),
    /// Winning solver mined reverted transaction.
    Revert(TransactionHash),
    /// Transaction started reverting during the submission.
    SimulationRevert,
    /// Winning solver failed to settle the transaction onchain.
    Fail,
}
