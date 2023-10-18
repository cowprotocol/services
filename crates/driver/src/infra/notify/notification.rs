use crate::domain::competition;

/// A notification is sent to the solvers in case a solution failed validation.
#[derive(Debug)]
pub struct Notification {
    pub auction_id: Option<competition::auction::Id>,
    pub kind: Kind,
}

#[derive(Debug)]
pub enum Kind {
    /// The solution doesn't contain any user orders.
    EmptySolution,
    /// No valid score could be computed for the solution.
    ScoringFailed,
    /// Solution aimed to internalize untrusted token/s.
    NonBufferableTokensUsed,
    /// Solver don't have enough balance to submit the solution.
    InsufficientBalance,
}
