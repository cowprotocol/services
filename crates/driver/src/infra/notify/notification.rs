use {
    crate::domain::{
        competition::{auction, solution},
        eth::{self, Ether, TokenAddress},
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
    /// The solver has been banned for a specific reason.
    Banned(BanReason),
}

#[derive(Debug)]
pub enum ScoreKind {
    /// No clearing prices are present for all trades.
    InvalidClearingPrices,
    /// The amount executed is invalid: out of range or the fee doesn't match
    /// its execution with fee
    InvalidExecutedAmount,
    /// missing native price for the surplus token
    MissingPrice(TokenAddress),
}

#[derive(Debug)]
pub enum BanReason {
    /// The driver won multiple consecutive auctions but never settled them.
    UnsettledConsecutiveAuctions,
}

#[derive(Debug)]
pub enum Settlement {
    /// Winning solver settled successfully transaction onchain.
    Success(TransactionHash),
    /// Winning solver mined reverted transaction.
    Revert(TransactionHash),
    /// Transaction started reverting during the submission.
    SimulationRevert,
    /// Transaction was not confirmed in time
    Expired,
    /// Winning solver failed to settle the transaction onchain for other
    /// reasons.
    Fail,
}
