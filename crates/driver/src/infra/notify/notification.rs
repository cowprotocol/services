use {
    crate::domain::{
        competition::{auction, solution},
        eth::{self, Ether, TokenAddress},
    },
    std::collections::BTreeSet,
};

/// A notification is sent to the solvers in case a solution failed validation.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: Option<auction::Id>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    /// The solution doesn't contain any user orders.
    EmptySolution(solution::Id),
    /// No valid score could be computed for the solution.
    ScoringFailed,
    /// Solution aimed to internalize tokens that are not considered safe to
    /// keep in the settlement contract.
    NonBufferableTokensUsed(BTreeSet<TokenAddress>),
    /// Solver don't have enough balance to submit the solution onchain.
    SolverAccountInsufficientBalance(Ether),
    /// Result of winning solver trying to settle the transaction onchain.
    Settled(Settlement),
}

#[derive(Debug)]
pub enum Settlement {
    /// Winning solver settled successfully transaction onchain.
    Success(eth::TxId),
    /// Winning solver mined reverted transaction.
    Revert(eth::TxId),
    /// Winning solver failed to settle the transaction onchain.
    Fail,
}
