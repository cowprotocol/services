use {
    crate::domain::{
        competition::{auction, score::Quality, solution, Score},
        eth::{self, Ether, GasCost, TokenAddress},
    },
    std::collections::BTreeSet,
};

type RequiredEther = Ether;
type TokensUsed = BTreeSet<TokenAddress>;
type TransactionHash = eth::TxId;
type Transaction = eth::Tx;
pub type SimulationSucceededAtLeastOnce = bool;

/// A notification sent to solvers in case of important events in the driver.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: Option<auction::Id>,
    pub solution_id: Option<solution::Id>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    /// Solver engine timed out.
    Timeout,
    /// The solution doesn't contain any user orders.
    EmptySolution,
    /// Solution received from solver engine don't have unique id.
    DuplicatedSolutionId,
    /// Failed simulation during competition. Last parameter is true
    /// if has simulated at least once.
    SimulationFailed(eth::BlockNo, Transaction, SimulationSucceededAtLeastOnce),
    /// No valid score could be computed for the solution.
    ScoringFailed(ScoreKind),
    /// Solution aimed to internalize tokens that are not considered safe to
    /// keep in the settlement contract.
    NonBufferableTokensUsed(TokensUsed),
    /// Solver don't have enough balance to submit the solution onchain.
    SolverAccountInsufficientBalance(RequiredEther),
    /// Result of winning solver trying to settle the transaction onchain.
    Settled(Settlement),
    /// Some aspect of the driver logic failed preventing the solution from
    /// participating in the auction.
    DriverError(String),
    /// On-chain solution postprocessing timed out.
    PostprocessingTimedOut,
}

#[derive(Debug)]
pub enum ScoreKind {
    /// The solution has zero score. Zero score solutions are not allowed as per
    /// CIP20 definition. The main reason being that reference score is zero,
    /// and if only one solution is in competition with zero score, that
    /// solution would receive 0 reward (reward = score - reference score).
    ZeroScore,
    /// Protocol does not allow solutions that are claimed to be "better" than
    /// the actual value they bring (quality). It is expected that score
    /// is always lower than quality, because there is always some
    /// execution cost that needs to be incorporated into the score and lower
    /// it.
    ScoreHigherThanQuality(Score, Quality),
    /// Solution has success probability that is outside of the allowed range
    /// [0, 1]
    /// [ONLY APPLICABLE TO SCORES BASED ON SUCCESS PROBABILITY]
    SuccessProbabilityOutOfRange(f64),
    /// Objective value is defined as quality (surplus + fees) - gas costs.
    /// Protocol doesn't allow solutions that cost more than they bring to
    /// the users and protocol.
    /// [ONLY APPLICABLE TO SCORES BASED ON SUCCESS PROBABILITY]
    ObjectiveValueNonPositive(Quality, GasCost),
}

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
